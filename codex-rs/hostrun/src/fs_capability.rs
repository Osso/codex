use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use serde_json::json;

use super::HostrunApprovalRequest;
use super::HostrunSessionError;

pub(super) fn fs_approval(tool_path: &str, args: Value) -> HostrunApprovalRequest {
    match tool_path {
        "fs.write" => fs_write_approval(args),
        "fs.read" => fs_path_approval("fs.read", "Read", args),
        "fs.exists" => fs_path_approval("fs.exists", "Check existence of", args),
        "fs.remove" => fs_path_approval("fs.remove", "Remove", args),
        _ => unreachable!("fs_approval only handles fs capabilities"),
    }
}

pub(super) fn execute_fs_operation(
    tool_path: &str,
    args: Value,
) -> Result<Value, HostrunSessionError> {
    match tool_path {
        "fs.write" => execute_fs_write(args),
        "fs.read" => execute_fs_read(args),
        "fs.exists" => Ok(Value::Bool(fs_path(&args).exists())),
        "fs.remove" => execute_fs_remove(args),
        _ => Err(HostrunSessionError::Eval(format!(
            "unsupported fs operation: {tool_path}"
        ))),
    }
}

fn fs_write_approval(args: Value) -> HostrunApprovalRequest {
    let path = field_as_string(&args, "path");
    let content = field_as_string(&args, "content");
    HostrunApprovalRequest {
        id: format!("fs.write:{path}"),
        tool: "fs.write".to_string(),
        summary: format!("Write {} bytes to {path}", content.len()),
        args,
    }
}

fn fs_path_approval(tool: &str, verb: &str, args: Value) -> HostrunApprovalRequest {
    let path = field_as_string(&args, "path");
    HostrunApprovalRequest {
        id: format!("{tool}:{path}"),
        tool: tool.to_string(),
        summary: format!("{verb} {path}"),
        args,
    }
}

fn execute_fs_write(args: Value) -> Result<Value, HostrunSessionError> {
    let path = fs_path(&args);
    let content = field_as_string(&args, "content");
    fs::write(&path, content.as_bytes()).map_err(|error| {
        HostrunSessionError::Eval(format!("failed to write {}: {error}", path.display()))
    })?;
    Ok(json!({
        "path": path,
        "bytes": content.len()
    }))
}

fn execute_fs_read(args: Value) -> Result<Value, HostrunSessionError> {
    let path = fs_path(&args);
    fs::read_to_string(&path)
        .map(Value::String)
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to read {}: {error}", path.display()))
        })
}

fn execute_fs_remove(args: Value) -> Result<Value, HostrunSessionError> {
    let path = fs_path(&args);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(json!({
                "path": path,
                "removed": false
            }));
        }
        Err(error) => {
            return Err(HostrunSessionError::Eval(format!(
                "failed to inspect {}: {error}",
                path.display()
            )));
        }
    };
    if metadata.is_dir() {
        fs::remove_dir_all(&path).map_err(|error| {
            HostrunSessionError::Eval(format!("failed to remove {}: {error}", path.display()))
        })?;
    } else {
        fs::remove_file(&path).map_err(|error| {
            HostrunSessionError::Eval(format!("failed to remove {}: {error}", path.display()))
        })?;
    }
    Ok(json!({
        "path": path,
        "removed": true
    }))
}

fn fs_path(args: &Value) -> PathBuf {
    PathBuf::from(field_as_string(args, "path"))
}

fn field_as_string(args: &Value, field: &str) -> String {
    args.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
