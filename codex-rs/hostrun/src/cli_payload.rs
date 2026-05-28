use serde_json::Value;
use serde_json::json;

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
    let mut payload = json!({});
    if let (Value::Object(payload), Some(Value::Object(io))) = (&mut payload, io) {
        payload.extend(io);
    }
    if let Value::Object(payload) = &mut payload {
        payload.insert("program".to_string(), Value::String(program.to_string()));
        payload.insert("args".to_string(), Value::Array(args));
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
