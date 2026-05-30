use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::ChildStderr;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;

use serde_json::Value;

use crate::cli_payload;
use crate::fs_capability::resolve_path;
use crate::session::HostrunSessionError;

pub(crate) struct CliStreamSource {
    pub(crate) stream: String,
    pub(crate) program: String,
    pub(crate) argv: Vec<String>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) env: Vec<(String, String)>,
}

pub(crate) fn stream_source(
    source: Option<&Value>,
) -> Result<CliStreamSource, HostrunSessionError> {
    let source = source
        .ok_or_else(|| HostrunSessionError::Eval("stdin stream source is required".to_string()))?;
    let stream = source
        .get("stream")
        .and_then(Value::as_str)
        .unwrap_or("stdout")
        .to_string();
    let command = stream_source_command(source)?;
    let program = command
        .get("program")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            HostrunSessionError::Eval("stdin stream command program is required".to_string())
        })?
        .to_string();
    Ok(CliStreamSource {
        stream,
        program,
        argv: payload_args(command)?,
        cwd: command
            .get("cwd")
            .and_then(Value::as_str)
            .map(PathBuf::from),
        env: cli_payload::payload_env(command)?,
    })
}

fn stream_source_command(
    source: &Value,
) -> Result<&serde_json::Map<String, Value>, HostrunSessionError> {
    let command = source
        .get("command")
        .ok_or_else(|| HostrunSessionError::Eval("stdin stream command is required".to_string()))?;
    let Value::Object(command) = command else {
        return Err(HostrunSessionError::Eval(
            "stdin stream command must be an object".to_string(),
        ));
    };
    Ok(command)
}

pub(crate) fn spawn_stream_source(
    source: &CliStreamSource,
    cwd: &Path,
) -> Result<Child, HostrunSessionError> {
    let cwd = source
        .cwd
        .as_ref()
        .map(|path| resolve_path(cwd, path))
        .unwrap_or_else(|| cwd.to_path_buf());
    let mut command = Command::new(&source.program);
    command
        .args(&source.argv)
        .current_dir(&cwd)
        .envs(source.env.iter().cloned());
    command
        .stdin(Stdio::null())
        .stdout(if source.stream == "stdout" {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(if source.stream == "stderr" {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .spawn()
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to start {}: {error}", source.program))
        })
}

pub(crate) fn take_stream_pipe(
    upstream: &mut Child,
    source: &CliStreamSource,
) -> Result<Stdio, HostrunSessionError> {
    match source.stream.as_str() {
        "stdout" => upstream
            .stdout
            .take()
            .map(ChildStdout::into)
            .ok_or_else(|| HostrunSessionError::Eval("failed to open upstream stdout".to_string())),
        "stderr" => upstream
            .stderr
            .take()
            .map(ChildStderr::into)
            .ok_or_else(|| HostrunSessionError::Eval("failed to open upstream stderr".to_string())),
        other => Err(HostrunSessionError::Eval(format!(
            "unsupported stdin stream source: {other}"
        ))),
    }
}

fn payload_args(
    payload: &serde_json::Map<String, Value>,
) -> Result<Vec<String>, HostrunSessionError> {
    let values = payload
        .get("args")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    values.iter().map(arg_to_string).collect()
}

fn arg_to_string(value: &Value) -> Result<String, HostrunSessionError> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null => Ok(String::new()),
        Value::Array(_) | Value::Object(_) => Err(HostrunSessionError::Eval(format!(
            "cli arguments must be scalar argv values, got {value}"
        ))),
    }
}
