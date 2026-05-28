use serde_json::Value;

pub(crate) fn io_summary(io: Option<&Value>) -> String {
    let Some(Value::Object(io)) = io else {
        return String::new();
    };
    let mut parts = Vec::new();
    if let Some(cwd) = io.get("cwd").and_then(Value::as_str) {
        parts.push(format!("cwd {cwd}"));
    }
    if let Some(stdin) = io.get("stdin") {
        parts.push(stdin_summary(stdin));
    }
    for name in ["stdout", "stderr", "combined"] {
        if let Some(output) = io.get(name) {
            parts.push(output_summary(name, output));
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    }
}

fn stdin_summary(stdin: &Value) -> String {
    match field_as_string(stdin, "type").as_str() {
        "stream" => stream_summary(stdin.get("source")),
        "file" => format!("stdin from {}", field_as_string(stdin, "path")),
        other => format!("stdin {other}"),
    }
}

fn stream_summary(source: Option<&Value>) -> String {
    let Some(source) = source else {
        return "stdin from stream".to_string();
    };
    let stream = source
        .get("stream")
        .and_then(Value::as_str)
        .unwrap_or("stdout");
    let Some(Value::Object(command)) = source.get("command") else {
        return format!("stdin from {stream}");
    };
    let program = command.get("program").and_then(Value::as_str).unwrap_or("");
    let args = command
        .get("args")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    format!("stdin from {} {stream}", command_summary(program, &args))
}

fn output_summary(name: &str, output: &Value) -> String {
    match field_as_string(output, "type").as_str() {
        "file" => format!("{name} to {}", field_as_string(output, "path")),
        "tee" => format!("{name} tee {}", field_as_string(output, "path")),
        "stdout" => "stderr to stdout".to_string(),
        other => format!("{name} {other}"),
    }
}

pub(crate) fn command_summary(program: &str, args: &[Value]) -> String {
    let mut parts = vec![program.to_string()];
    parts.extend(args.iter().map(arg_summary));
    parts.join(" ")
}

fn arg_summary(arg: &Value) -> String {
    match arg {
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn field_as_string(args: &Value, field: &str) -> String {
    args.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
