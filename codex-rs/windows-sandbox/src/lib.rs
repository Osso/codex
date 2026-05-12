//! No-op compatibility crate for removed Windows sandbox support.

use std::path::Path;
use std::path::PathBuf;

use codex_protocol::permissions::FileSystemSandboxPolicy;

pub fn resolve_windows_deny_read_paths(
    _file_system_sandbox_policy: &FileSystemSandboxPolicy,
    _sandbox_policy_cwd: &Path,
) -> Result<Vec<PathBuf>, String> {
    Ok(Vec::new())
}

pub fn apply_world_writable_scan_and_denies() -> Result<(), String> {
    Err("Windows sandbox setup is not supported by this fork".to_string())
}
