use std::fs;
use std::path::Path;
use std::path::PathBuf;

use glob::glob;
use serde_json::Value;
use serde_json::json;

use super::HostrunApprovalRequest;
use super::HostrunSessionError;

pub(super) fn fs_approval(tool_path: &str, args: Value, cwd: &Path) -> HostrunApprovalRequest {
    match tool_path {
        "fs.write" => fs_write_approval(args, cwd),
        "fs.read" => fs_path_approval("fs.read", "Read", args, cwd),
        "fs.exists" => fs_path_approval("fs.exists", "Check existence of", args, cwd),
        "fs.remove" => fs_path_approval("fs.remove", "Remove", args, cwd),
        "fs.glob" => fs_glob_approval(args, cwd),
        _ => unreachable!("fs_approval only handles fs capabilities"),
    }
}

pub(super) fn execute_fs_operation(
    tool_path: &str,
    args: Value,
    cwd: &Path,
) -> Result<Value, HostrunSessionError> {
    match tool_path {
        "fs.write" => execute_fs_write(args, cwd),
        "fs.read" => execute_fs_read(args, cwd),
        "fs.exists" => Ok(Value::Bool(fs_path(&args, cwd).exists())),
        "fs.remove" => execute_fs_remove(args, cwd),
        "fs.glob" => execute_fs_glob(args, cwd),
        _ => Err(HostrunSessionError::Eval(format!(
            "unsupported fs operation: {tool_path}"
        ))),
    }
}

fn fs_write_approval(mut args: Value, cwd: &Path) -> HostrunApprovalRequest {
    let path = fs_path(&args, cwd);
    set_path_field(&mut args, "path", &path);
    let content = field_as_string(&args, "content");
    let path_display = path.display();
    HostrunApprovalRequest {
        id: format!("fs.write:{path_display}"),
        tool: "fs.write".to_string(),
        summary: format!("Write {} bytes to {path_display}", content.len()),
        args,
    }
}

fn fs_path_approval(tool: &str, verb: &str, mut args: Value, cwd: &Path) -> HostrunApprovalRequest {
    let path = fs_path(&args, cwd);
    set_path_field(&mut args, "path", &path);
    let path_display = path.display();
    HostrunApprovalRequest {
        id: format!("{tool}:{path_display}"),
        tool: tool.to_string(),
        summary: format!("{verb} {path_display}"),
        args,
    }
}

fn fs_glob_approval(mut args: Value, cwd: &Path) -> HostrunApprovalRequest {
    let pattern = glob_pattern(&args, cwd);
    set_string_field(&mut args, "pattern", pattern.to_string_lossy().as_ref());
    HostrunApprovalRequest {
        id: format!("fs.glob:{}", pattern.display()),
        tool: "fs.glob".to_string(),
        summary: format!("Glob {}", pattern.display()),
        args,
    }
}

fn execute_fs_write(args: Value, cwd: &Path) -> Result<Value, HostrunSessionError> {
    let path = fs_path(&args, cwd);
    let content = field_as_string(&args, "content");
    fs::write(&path, content.as_bytes()).map_err(|error| {
        HostrunSessionError::Eval(format!("failed to write {}: {error}", path.display()))
    })?;
    Ok(json!({
        "path": path,
        "bytes": content.len()
    }))
}

fn execute_fs_read(args: Value, cwd: &Path) -> Result<Value, HostrunSessionError> {
    let path = fs_path(&args, cwd);
    fs::read_to_string(&path)
        .map(Value::String)
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to read {}: {error}", path.display()))
        })
}

fn execute_fs_remove(args: Value, cwd: &Path) -> Result<Value, HostrunSessionError> {
    let path = fs_path(&args, cwd);
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

fn execute_fs_glob(args: Value, cwd: &Path) -> Result<Value, HostrunSessionError> {
    let pattern = glob_pattern(&args, cwd);
    let options = args.get("options").unwrap_or(&Value::Null);
    let entry_type = options.get("type").and_then(Value::as_str);
    let mut paths = Vec::new();
    for entry in glob(pattern.to_string_lossy().as_ref()).map_err(|error| {
        HostrunSessionError::Eval(format!(
            "invalid glob pattern {}: {error}",
            pattern.display()
        ))
    })? {
        let path = entry.map_err(|error| {
            HostrunSessionError::Eval(format!(
                "failed to read glob entry for {}: {error}",
                pattern.display()
            ))
        })?;
        if glob_entry_matches_type(&path, entry_type) {
            paths.push(path.to_string_lossy().to_string());
        }
    }
    paths.sort();
    Ok(json!(paths))
}

fn glob_entry_matches_type(path: &Path, entry_type: Option<&str>) -> bool {
    match entry_type {
        Some("file") | Some("files") => path.is_file(),
        Some("dir") | Some("dirs") | Some("directory") | Some("directories") => path.is_dir(),
        _ => true,
    }
}

pub(crate) fn resolve_path(cwd: &Path, path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn fs_path(args: &Value, cwd: &Path) -> PathBuf {
    resolve_path(cwd, field_as_string(args, "path"))
}

fn glob_pattern(args: &Value, cwd: &Path) -> PathBuf {
    resolve_path(cwd, field_as_string(args, "pattern"))
}

fn set_path_field(args: &mut Value, field: &str, path: &Path) {
    set_string_field(args, field, path.to_string_lossy().as_ref());
}

fn set_string_field(args: &mut Value, field: &str, value: &str) {
    if let Value::Object(args) = args {
        args.insert(field.to_string(), Value::String(value.to_string()));
    }
}

fn field_as_string(args: &Value, field: &str) -> String {
    args.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
