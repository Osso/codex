use std::path::Path;
use std::path::PathBuf;

use serde_json::Map;
use serde_json::Value;
use serde_json::json;

use crate::fs_capability::resolve_path;
use crate::session::HostrunSessionError;

pub(crate) fn split_command_payload(args: Value) -> (Vec<Value>, Option<Value>) {
    match args {
        Value::Array(args) => (args, None),
        Value::Object(mut payload) if payload.contains_key("args") => {
            let cli_args = match payload.remove("args").unwrap_or(Value::Null) {
                Value::Array(args) => args,
                Value::Null => Vec::new(),
                other => vec![other],
            };
            if payload.is_empty() {
                (cli_args, None)
            } else {
                (cli_args, Some(Value::Object(payload)))
            }
        }
        Value::Null => (Vec::new(), None),
        other => (vec![other], None),
    }
}

pub(crate) fn command_args(program: &str, args: Vec<Value>, io: Option<Value>) -> Value {
    let mut payload = json!({
        "program": program,
        "args": args,
    });
    if let (Value::Object(payload), Some(Value::Object(io))) = (&mut payload, io) {
        payload.extend(io);
    }
    payload
}

pub(crate) fn payload_args(
    payload: &serde_json::Map<String, Value>,
) -> Result<Vec<String>, HostrunSessionError> {
    let values = payload
        .get("args")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    values.iter().map(arg_to_string).collect()
}

pub(crate) fn payload_cwd(
    payload: &serde_json::Map<String, Value>,
    session_cwd: &Path,
) -> Result<PathBuf, HostrunSessionError> {
    let Some(cwd) = payload.get("cwd") else {
        return Ok(session_cwd.to_path_buf());
    };
    let Some(cwd) = cwd.as_str() else {
        return Err(HostrunSessionError::Eval(
            "cli cwd must be a string".to_string(),
        ));
    };
    Ok(resolve_path(session_cwd, cwd))
}

pub(crate) fn payload_env(
    payload: &serde_json::Map<String, Value>,
) -> Result<Vec<(String, String)>, HostrunSessionError> {
    let Some(env) = payload.get("env") else {
        return Ok(Vec::new());
    };
    let Value::Object(env) = env else {
        return Err(HostrunSessionError::Eval(
            "cli env must be an object".to_string(),
        ));
    };
    env.iter()
        .map(|(key, value)| Ok((key.clone(), arg_to_string(value)?)))
        .collect()
}

pub(crate) fn redact_env_values(payload: &mut Value) {
    let Value::Object(payload) = payload else {
        return;
    };
    let Some(Value::Object(env)) = payload.get_mut("env") else {
        return;
    };
    let redacted = env
        .keys()
        .map(|key| (key.clone(), Value::String("[redacted]".to_string())))
        .collect::<Map<String, Value>>();
    *env = redacted;
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
