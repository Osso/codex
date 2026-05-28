use std::fs;
use std::path::Path;

use serde_json::Value;
use serde_json::json;

use crate::fs_capability::resolve_path;
use crate::session::HostrunSessionError;

const CAPTURE_LIMIT_BYTES: usize = 64 * 1024;

pub(crate) fn apply_output_intent(
    result: &mut serde_json::Map<String, Value>,
    name: &str,
    intent: Option<&Value>,
    bytes: &[u8],
    cwd: &Path,
) -> Result<(), HostrunSessionError> {
    let Some(intent) = intent else {
        return Ok(());
    };
    match intent
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("capture")
    {
        "capture" | "text" => capture_output(result, name, bytes),
        "lines" => capture_lines(result, name, bytes),
        "file" => write_output_file(result, name, intent, bytes, cwd),
        "tee" => tee_output(result, name, intent, bytes, cwd),
        other => Err(HostrunSessionError::Eval(format!(
            "unsupported {name} output intent: {other}"
        ))),
    }
}

fn capture_output(
    result: &mut serde_json::Map<String, Value>,
    name: &str,
    bytes: &[u8],
) -> Result<(), HostrunSessionError> {
    let captured = bounded_capture(bytes);
    result.insert(
        name.to_string(),
        Value::String(String::from_utf8_lossy(captured).to_string()),
    );
    insert_capture_metadata(result, name, bytes.len(), captured.len());
    Ok(())
}

fn capture_lines(
    result: &mut serde_json::Map<String, Value>,
    name: &str,
    bytes: &[u8],
) -> Result<(), HostrunSessionError> {
    let captured = bounded_capture(bytes);
    let text = String::from_utf8_lossy(captured);
    result.insert(name.to_string(), json!(text.lines().collect::<Vec<_>>()));
    insert_capture_metadata(result, name, bytes.len(), captured.len());
    Ok(())
}

fn write_output_file(
    result: &mut serde_json::Map<String, Value>,
    name: &str,
    intent: &Value,
    bytes: &[u8],
    cwd: &Path,
) -> Result<(), HostrunSessionError> {
    let path = resolve_path(cwd, field_as_string(intent, "path"));
    fs::write(&path, bytes).map_err(|error| {
        HostrunSessionError::Eval(format!(
            "failed to write {name} to {}: {error}",
            path.display()
        ))
    })?;
    result.insert(
        name.to_string(),
        json!({ "path": path, "bytes": bytes.len() }),
    );
    Ok(())
}

fn tee_output(
    result: &mut serde_json::Map<String, Value>,
    name: &str,
    intent: &Value,
    bytes: &[u8],
    cwd: &Path,
) -> Result<(), HostrunSessionError> {
    let path = resolve_path(cwd, field_as_string(intent, "path"));
    fs::write(&path, bytes).map_err(|error| {
        HostrunSessionError::Eval(format!(
            "failed to tee {name} to {}: {error}",
            path.display()
        ))
    })?;
    capture_output(result, name, bytes)?;
    result.insert(
        format!("{name}File"),
        json!({ "path": path, "bytes": bytes.len() }),
    );
    Ok(())
}

fn bounded_capture(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.len().min(CAPTURE_LIMIT_BYTES)]
}

fn insert_capture_metadata(
    result: &mut serde_json::Map<String, Value>,
    name: &str,
    bytes: usize,
    captured_bytes: usize,
) {
    result.insert(
        format!("{name}Meta"),
        json!({
            "bytes": bytes,
            "capturedBytes": captured_bytes,
            "truncated": captured_bytes < bytes
        }),
    );
}

fn field_as_string(args: &Value, field: &str) -> String {
    args.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
