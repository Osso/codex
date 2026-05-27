use std::fs;
use std::io::Write;
use std::process::Child;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Output;
use std::process::Stdio;

use serde_json::Value;
use serde_json::json;

use crate::cli_graph::insert_command_graph;
use crate::cli_payload;
use crate::cli_stream;
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
) -> Result<CliProcessOutput, HostrunSessionError> {
    if stdin.and_then(stdin_type) == Some("stream") {
        return run_stream_cli_process(program, argv, stdin.and_then(|stdin| stdin.get("source")));
    }
    let stdin = stdin_input(stdin)?;
    let mut child = spawn_cli_process(program, argv, stdin.bytes.is_some())?;
    write_cli_stdin(program, &mut child, stdin.bytes)?;
    let output = child.wait_with_output().map_err(|error| {
        HostrunSessionError::Eval(format!("failed to wait for {program}: {error}"))
    })?;
    Ok(cli_process_output(program, argv, stdin.upstream, output))
}

pub(crate) fn stdin_type(stdin: &Value) -> Option<&str> {
    stdin.get("type").and_then(Value::as_str)
}

pub(crate) fn spawn_cli_process(
    program: &str,
    argv: &[String],
    has_stdin: bool,
) -> Result<Child, HostrunSessionError> {
    Command::new(program)
        .args(argv)
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

pub(crate) fn write_cli_stdin(
    program: &str,
    child: &mut Child,
    input: Option<Vec<u8>>,
) -> Result<(), HostrunSessionError> {
    let Some(input) = input else {
        return Ok(());
    };
    let child_stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| HostrunSessionError::Eval(format!("failed to open stdin for {program}")))?;
    child_stdin.write_all(&input).map_err(|error| {
        HostrunSessionError::Eval(format!("failed to write stdin for {program}: {error}"))
    })
}

pub(crate) fn stdin_input(stdin: Option<&Value>) -> Result<CliStdinInput, HostrunSessionError> {
    let Some(stdin) = stdin else {
        return Ok(CliStdinInput {
            bytes: None,
            upstream: Vec::new(),
        });
    };
    let (bytes, upstream) = stdin_bytes(stdin)?;
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
    apply_output_intent(&mut result, "stdout", payload.get("stdout"), &stdout)?;
    if !stderr_to_stdout {
        apply_output_intent(&mut result, "stderr", payload.get("stderr"), &output.stderr)?;
    }
    if let Some(combined) = payload.get("combined") {
        let mut bytes = output.stdout.clone();
        bytes.extend_from_slice(&output.stderr);
        apply_output_intent(&mut result, "combined", Some(combined), &bytes)?;
    }
    Ok(Value::Object(result))
}

fn run_stream_cli_process(
    program: &str,
    argv: &[String],
    source: Option<&Value>,
) -> Result<CliProcessOutput, HostrunSessionError> {
    let source = cli_stream::stream_source(source)?;
    let mut upstream = cli_stream::spawn_stream_source(&source)?;
    let pipe = cli_stream::take_stream_pipe(&mut upstream, &source)?;
    let output = Command::new(program)
        .args(argv)
        .stdin(pipe)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| HostrunSessionError::Eval(format!("failed to start {program}: {error}")))?
        .wait_with_output()
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to wait for {program}: {error}"))
        })?;
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

fn cli_command_status(program: &str, argv: &[String], status: ExitStatus) -> CliCommandStatus {
    CliCommandStatus {
        program: program.to_string(),
        args: argv.to_vec(),
        exit_code: status.code(),
        success: status.success(),
    }
}

fn stdin_bytes(stdin: &Value) -> Result<(Vec<u8>, Vec<CliCommandStatus>), HostrunSessionError> {
    let stdin_type = stdin
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("stream");
    let bytes = match stdin_type {
        "text" => field_as_string(stdin, "text").into_bytes(),
        "file" => fs::read(field_as_string(stdin, "path")).map_err(|error| {
            HostrunSessionError::Eval(format!("failed to read stdin file: {error}"))
        })?,
        "json" => serialize_json_stdin(stdin.get("value").unwrap_or(&Value::Null))?,
        "yaml" => serialize_yaml_stdin(stdin.get("value").unwrap_or(&Value::Null))?,
        "csv" => serialize_delimited_rows(stdin.get("rows"), ",")?,
        "tsv" => serialize_delimited_rows(stdin.get("rows"), "\t")?,
        "jsonLines" => serialize_json_lines(stdin.get("values"))?,
        "lines" => serialize_lines(stdin.get("lines"))?,
        "stream" => return stream_stdin_bytes(stdin.get("source")),
        other => Err(HostrunSessionError::Eval(format!(
            "unsupported stdin source type: {other}"
        )))?,
    };
    Ok((bytes, Vec::new()))
}

fn stream_stdin_bytes(
    source: Option<&Value>,
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
    let output = run_cli_process(program, &argv, None)?;
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
