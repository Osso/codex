use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Output;
use std::process::Stdio;
use std::thread;
use std::time::Duration;

use serde_json::Value;
use serde_json::json;

use crate::cli_graph::insert_command_graph;
use crate::cli_payload;
use crate::cli_stream;
use crate::execution_context::HostrunExecutionContext;
use crate::execution_context::HostrunOutputDelta;
use crate::fs_capability::resolve_path;
use crate::output_intent::apply_output_intent;
use crate::session::HostrunSessionError;

pub(crate) struct CliProcessOutput {
    pub(crate) command: CliCommandStatus,
    pub(crate) upstream: Vec<CliCommandStatus>,
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

pub(crate) struct CliCommandStatus {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) success: bool,
}

pub(crate) struct CliStdinInput {
    pub(crate) bytes: Option<Vec<u8>>,
    upstream: Vec<CliCommandStatus>,
}

pub(crate) fn run_cli_process(
    program: &str,
    argv: &[String],
    stdin: Option<&Value>,
    env: Option<&Value>,
    cwd: &Path,
    context: &HostrunExecutionContext,
) -> Result<CliProcessOutput, HostrunSessionError> {
    if stdin.and_then(stdin_type) == Some("stream") {
        return run_stream_cli_process(
            program,
            argv,
            stdin.and_then(|stdin| stdin.get("source")),
            env,
            cwd,
            context,
        );
    }
    let stdin = stdin_input(stdin, cwd, context)?;
    let env = command_env(env)?;
    let mut child = spawn_cli_process(program, argv, stdin.bytes.is_some(), &env, cwd)?;
    write_cli_stdin(program, &mut child, stdin.bytes)?;
    let output = wait_with_live_output(program, child, context)?;
    Ok(cli_process_output(program, argv, stdin.upstream, output))
}

pub(crate) fn stdin_type(stdin: &Value) -> Option<&str> {
    stdin.get("type").and_then(Value::as_str)
}

pub(crate) fn spawn_cli_process(
    program: &str,
    argv: &[String],
    has_stdin: bool,
    env: &[(String, String)],
    cwd: &Path,
) -> Result<Child, HostrunSessionError> {
    let mut command = Command::new(program);
    command
        .args(argv)
        .current_dir(cwd)
        .envs(env.iter().cloned());
    command
        .stdin(if has_stdin {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| HostrunSessionError::Eval(format!("failed to start {program}: {error}")))
}

pub(crate) fn command_env(
    env: Option<&Value>,
) -> Result<Vec<(String, String)>, HostrunSessionError> {
    let Some(Value::Object(payload)) = env.map(|env| json!({ "env": env })) else {
        return Ok(Vec::new());
    };
    cli_payload::payload_env(&payload)
}

pub(crate) fn write_cli_stdin(
    program: &str,
    child: &mut Child,
    input: Option<Vec<u8>>,
) -> Result<(), HostrunSessionError> {
    let Some(input) = input else {
        return Ok(());
    };
    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| HostrunSessionError::Eval(format!("failed to open stdin for {program}")))?;
    child_stdin.write_all(&input).map_err(|error| {
        HostrunSessionError::Eval(format!("failed to write stdin for {program}: {error}"))
    })
}

pub(crate) fn stdin_input(
    stdin: Option<&Value>,
    cwd: &Path,
    context: &HostrunExecutionContext,
) -> Result<CliStdinInput, HostrunSessionError> {
    let Some(stdin) = stdin else {
        return Ok(CliStdinInput {
            bytes: None,
            upstream: Vec::new(),
        });
    };
    let (bytes, upstream) = stdin_bytes(stdin, cwd, context)?;
    Ok(CliStdinInput {
        bytes: Some(bytes),
        upstream,
    })
}

pub(crate) fn cli_process_output(
    program: &str,
    argv: &[String],
    upstream: Vec<CliCommandStatus>,
    output: Output,
) -> CliProcessOutput {
    let command = cli_command_status(program, argv, output.status);
    let success = command.success && upstream.iter().all(|command| command.success);
    CliProcessOutput {
        command,
        upstream,
        success,
        stdout: output.stdout,
        stderr: output.stderr,
    }
}

pub(crate) fn cli_execution_result(
    program: &str,
    argv: &[String],
    payload: &serde_json::Map<String, Value>,
    output: CliProcessOutput,
    cwd: &Path,
) -> Result<Value, HostrunSessionError> {
    let mut result = serde_json::Map::new();
    result.insert("program".to_string(), Value::String(program.to_string()));
    result.insert("args".to_string(), json!(argv));
    result.insert("exitCode".to_string(), json!(output.command.exit_code));
    result.insert("success".to_string(), Value::Bool(output.success));
    insert_command_graph(&mut result, &output);
    let stderr_to_stdout = output_intent_type(payload.get("stderr")) == Some("stdout");
    let stdout = if stderr_to_stdout {
        let mut bytes = output.stdout.clone();
        bytes.extend_from_slice(&output.stderr);
        bytes
    } else {
        output.stdout.clone()
    };
    apply_output_intent(&mut result, "stdout", payload.get("stdout"), &stdout, cwd)?;
    if !stderr_to_stdout {
        apply_output_intent(
            &mut result,
            "stderr",
            payload.get("stderr"),
            &output.stderr,
            cwd,
        )?;
    }
    if let Some(combined) = payload.get("combined") {
        let mut bytes = output.stdout.clone();
        bytes.extend_from_slice(&output.stderr);
        apply_output_intent(&mut result, "combined", Some(combined), &bytes, cwd)?;
    }
    Ok(Value::Object(result))
}

fn run_stream_cli_process(
    program: &str,
    argv: &[String],
    source: Option<&Value>,
    env: Option<&Value>,
    cwd: &Path,
    context: &HostrunExecutionContext,
) -> Result<CliProcessOutput, HostrunSessionError> {
    let env = command_env(env)?;
    let source = cli_stream::stream_source(source)?;
    let mut upstream = cli_stream::spawn_stream_source(&source, cwd)?;
    let pipe = cli_stream::take_stream_pipe(&mut upstream, &source)?;
    let mut command = Command::new(program);
    command
        .args(argv)
        .current_dir(cwd)
        .envs(env.iter().cloned());
    let child = command
        .stdin(pipe)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to start {program}: {error}"))
        })?;
    let output = wait_with_live_output(program, child, context)?;
    let upstream_status = upstream.wait().map_err(|error| {
        HostrunSessionError::Eval(format!("failed to wait for {}: {error}", source.program))
    })?;
    let upstream = vec![cli_command_status(
        &source.program,
        &source.argv,
        upstream_status,
    )];
    Ok(cli_process_output(program, argv, upstream, output))
}

pub(crate) fn wait_with_live_output(
    program: &str,
    mut child: Child,
    context: &HostrunExecutionContext,
) -> Result<Output, HostrunSessionError> {
    let output_readers = spawn_output_readers(program, &mut child, context)?;
    let status = wait_for_child_status(program, &mut child, context)?;
    let stdout = join_output_reader(program, "stdout", output_readers.stdout)?;
    let stderr = join_output_reader(program, "stderr", output_readers.stderr)?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

struct OutputReaders {
    stdout: thread::JoinHandle<std::io::Result<Vec<u8>>>,
    stderr: thread::JoinHandle<std::io::Result<Vec<u8>>>,
}

fn spawn_output_readers(
    program: &str,
    child: &mut Child,
    context: &HostrunExecutionContext,
) -> Result<OutputReaders, HostrunSessionError> {
    let stdout = take_piped_output(child.stdout.take(), program, "stdout")?;
    let stderr = take_piped_output(child.stderr.take(), program, "stderr")?;
    let stdout_context = context.clone();
    let stderr_context = context.clone();
    Ok(OutputReaders {
        stdout: thread::spawn(move || {
            read_live_output(stdout, stdout_context, HostrunOutputDelta::stdout)
        }),
        stderr: thread::spawn(move || {
            read_live_output(stderr, stderr_context, HostrunOutputDelta::stderr)
        }),
    })
}

fn take_piped_output<T>(
    output: Option<T>,
    program: &str,
    stream: &str,
) -> Result<T, HostrunSessionError> {
    output.ok_or_else(|| HostrunSessionError::Eval(format!("{program} {stream} was not piped")))
}

fn wait_for_child_status(
    program: &str,
    child: &mut Child,
    context: &HostrunExecutionContext,
) -> Result<ExitStatus, HostrunSessionError> {
    let mut interrupted = false;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            HostrunSessionError::Eval(format!("failed to wait for {program}: {error}"))
        })? {
            break status;
        }
        if context.is_cancelled() {
            interrupted = true;
            child.kill().map_err(|error| {
                HostrunSessionError::Eval(format!("failed to interrupt {program}: {error}"))
            })?;
            break child.wait().map_err(|error| {
                HostrunSessionError::Eval(format!(
                    "failed to wait for interrupted {program}: {error}"
                ))
            })?;
        }
        thread::sleep(Duration::from_millis(10));
    };
    if interrupted {
        return Err(HostrunSessionError::Eval(format!(
            "{program} interrupted by user"
        )));
    }
    Ok(status)
}

fn read_live_output<R>(
    mut reader: R,
    context: HostrunExecutionContext,
    delta: fn(Vec<u8>) -> HostrunOutputDelta,
) -> std::io::Result<Vec<u8>>
where
    R: Read,
{
    let mut output = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        let chunk = buffer[..bytes_read].to_vec();
        context.emit_output(delta(chunk.clone()));
        output.extend_from_slice(&chunk);
    }
    Ok(output)
}

fn join_output_reader(
    program: &str,
    stream: &str,
    handle: thread::JoinHandle<std::io::Result<Vec<u8>>>,
) -> Result<Vec<u8>, HostrunSessionError> {
    handle
        .join()
        .map_err(|_| HostrunSessionError::Eval(format!("{program} {stream} reader panicked")))?
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to read {program} {stream}: {error}"))
        })
}

fn cli_command_status(program: &str, argv: &[String], status: ExitStatus) -> CliCommandStatus {
    CliCommandStatus {
        program: program.to_string(),
        args: argv.to_vec(),
        exit_code: status.code(),
        success: status.success(),
    }
}

fn stdin_bytes(
    stdin: &Value,
    cwd: &Path,
    context: &HostrunExecutionContext,
) -> Result<(Vec<u8>, Vec<CliCommandStatus>), HostrunSessionError> {
    let stdin_type = stdin
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("stream");
    let bytes = match stdin_type {
        "text" => field_as_string(stdin, "text").into_bytes(),
        "file" => fs::read(resolve_path(cwd, field_as_string(stdin, "path"))).map_err(|error| {
            HostrunSessionError::Eval(format!("failed to read stdin file: {error}"))
        })?,
        "json" => serialize_json_stdin(stdin.get("value").unwrap_or(&Value::Null))?,
        "yaml" => serialize_yaml_stdin(stdin.get("value").unwrap_or(&Value::Null))?,
        "csv" => serialize_delimited_rows(stdin.get("rows"), ",")?,
        "tsv" => serialize_delimited_rows(stdin.get("rows"), "\t")?,
        "jsonLines" => serialize_json_lines(stdin.get("values"))?,
        "lines" => serialize_lines(stdin.get("lines"))?,
        "stream" => return stream_stdin_bytes(stdin.get("source"), cwd, context),
        other => Err(HostrunSessionError::Eval(format!(
            "unsupported stdin source type: {other}"
        )))?,
    };
    Ok((bytes, Vec::new()))
}

fn stream_stdin_bytes(
    source: Option<&Value>,
    cwd: &Path,
    context: &HostrunExecutionContext,
) -> Result<(Vec<u8>, Vec<CliCommandStatus>), HostrunSessionError> {
    let source = source
        .ok_or_else(|| HostrunSessionError::Eval("stdin stream source is required".to_string()))?;
    let stream = source
        .get("stream")
        .and_then(Value::as_str)
        .unwrap_or("stdout");
    let command = source
        .get("command")
        .ok_or_else(|| HostrunSessionError::Eval("stdin stream command is required".to_string()))?;
    let Value::Object(command) = command else {
        return Err(HostrunSessionError::Eval(
            "stdin stream command must be an object".to_string(),
        ));
    };
    let program = command
        .get("program")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            HostrunSessionError::Eval("stdin stream command program is required".to_string())
        })?;
    let argv = cli_payload::payload_args(command)?;
    let output = run_cli_process(program, &argv, None, command.get("env"), cwd, context)?;
    let bytes = match stream {
        "stdout" => output.stdout,
        "stderr" => output.stderr,
        other => Err(HostrunSessionError::Eval(format!(
            "unsupported stdin stream source: {other}"
        )))?,
    };
    let mut upstream = output.upstream;
    upstream.push(output.command);
    Ok((bytes, upstream))
}

fn serialize_json_stdin(value: &Value) -> Result<Vec<u8>, HostrunSessionError> {
    serde_json::to_vec(value)
        .map(|mut bytes| {
            bytes.push(b'\n');
            bytes
        })
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to serialize JSON stdin: {error}"))
        })
}

fn serialize_yaml_stdin(value: &Value) -> Result<Vec<u8>, HostrunSessionError> {
    serde_yaml::to_string(value)
        .map(String::into_bytes)
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to serialize YAML stdin: {error}"))
        })
}

fn serialize_delimited_rows(
    rows: Option<&Value>,
    delimiter: &str,
) -> Result<Vec<u8>, HostrunSessionError> {
    let rows = rows
        .and_then(Value::as_array)
        .ok_or_else(|| HostrunSessionError::Eval("stdin rows must be an array".to_string()))?;
    let lines = rows
        .iter()
        .map(|row| {
            row.as_array()
                .ok_or_else(|| HostrunSessionError::Eval("stdin row must be an array".to_string()))
                .map(|cells| {
                    cells
                        .iter()
                        .map(stdin_cell_text)
                        .collect::<Vec<_>>()
                        .join(delimiter)
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((lines.join("\n") + "\n").into_bytes())
}

fn stdin_cell_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn serialize_json_lines(values: Option<&Value>) -> Result<Vec<u8>, HostrunSessionError> {
    let values = values.and_then(Value::as_array).ok_or_else(|| {
        HostrunSessionError::Eval("stdin JSON lines must be an array".to_string())
    })?;
    let lines = values
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to serialize JSONL stdin: {error}"))
        })?;
    Ok((lines.join("\n") + "\n").into_bytes())
}

fn serialize_lines(lines: Option<&Value>) -> Result<Vec<u8>, HostrunSessionError> {
    let lines = lines
        .and_then(Value::as_array)
        .ok_or_else(|| HostrunSessionError::Eval("stdin lines must be an array".to_string()))?;
    let text = lines
        .iter()
        .map(stdin_cell_text)
        .collect::<Vec<_>>()
        .join("\n");
    Ok((text + "\n").into_bytes())
}

fn output_intent_type(intent: Option<&Value>) -> Option<&str> {
    intent
        .and_then(|intent| intent.get("type"))
        .and_then(Value::as_str)
}

fn field_as_string(args: &Value, field: &str) -> String {
    args.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
