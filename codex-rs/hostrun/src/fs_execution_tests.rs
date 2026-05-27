use std::fs;

use serde_json::json;

use super::HostrunSession;

#[test]
fn public_fs_glob_returns_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("fs.glob('/tmp/hostrun-*.txt', { type: 'file' });")
        .expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.id, "fs.glob:/tmp/hostrun-*.txt");
    assert_eq!(approval.tool, "fs.glob");
    assert_eq!(approval.summary, "Glob /tmp/hostrun-*.txt");
    assert_eq!(
        approval.args,
        json!({ "pattern": "/tmp/hostrun-*.txt", "options": { "type": "file" } })
    );
}

#[test]
fn approved_fs_helpers_write_read_exist_and_remove_files() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("probe.txt");
    let path_text = path.to_string_lossy().to_string();

    let write = session
        .eval(&format!("fs.write({}, 'hello fs');", json!(path_text)))
        .expect("write");
    assert_eq!(
        write.value,
        Some(json!({
            "path": path_text,
            "bytes": 8
        }))
    );
    assert_eq!(fs::read_to_string(&path).expect("file content"), "hello fs");

    let read = session
        .eval(&format!("fs.read({});", json!(path_text)))
        .expect("read");
    assert_eq!(read.value, Some(json!("hello fs")));

    let exists = session
        .eval(&format!("fs.exists({});", json!(path_text)))
        .expect("exists");
    assert_eq!(exists.value, Some(json!(true)));

    let remove = session
        .eval(&format!("fs.remove({});", json!(path_text)))
        .expect("remove");
    assert_eq!(
        remove.value,
        Some(json!({
            "path": path_text,
            "removed": true
        }))
    );
    assert!(!path.exists());
}

#[test]
fn approved_structured_file_writes_execute_serialized_content() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("data.jsonl");
    let path_text = path.to_string_lossy().to_string();

    session
        .eval(&format!(
            "fs.writeJsonLines({}, [{{ ok: true }}, {{ name: 'alpha' }}]);",
            json!(path_text)
        ))
        .expect("write jsonl");

    assert_eq!(
        fs::read_to_string(&path).expect("jsonl content"),
        "{\"ok\":true}\n{\"name\":\"alpha\"}\n"
    );
}

#[test]
fn approved_fs_remove_reports_missing_path_without_error() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("missing.txt");
    let path_text = path.to_string_lossy().to_string();

    let remove = session
        .eval(&format!("fs.remove({});", json!(path_text)))
        .expect("remove missing");

    assert_eq!(
        remove.value,
        Some(json!({
            "path": path_text,
            "removed": false
        }))
    );
}

#[test]
fn approved_fs_glob_lists_matching_paths_and_open_parses_by_extension() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let alpha = dir.path().join("alpha.json");
    let beta = dir.path().join("beta.txt");
    let nested = dir.path().join("nested");
    fs::create_dir(&nested).expect("nested dir");
    fs::write(&alpha, r#"{"name":"alpha","ok":true}"#).expect("alpha json");
    fs::write(&beta, "beta").expect("beta txt");

    let pattern = dir.path().join("*.json").to_string_lossy().to_string();
    let result = session
        .eval(&format!("fs.glob({}, {{ type: 'file' }});", json!(pattern)))
        .expect("glob");
    assert_eq!(
        result.value,
        Some(json!([alpha.to_string_lossy().to_string()]))
    );

    let open = session
        .eval(&format!(
            "fs.open({});",
            json!(alpha.to_string_lossy().to_string())
        ))
        .expect("open");
    assert_eq!(
        open.value,
        Some(json!({
            "name": "alpha",
            "ok": true
        }))
    );

    let dirs = session
        .eval(&format!(
            "fs.glob({}, {{ type: 'dir' }});",
            json!(dir.path().join("*").to_string_lossy().to_string())
        ))
        .expect("glob dirs");
    assert_eq!(
        dirs.value,
        Some(json!([nested.to_string_lossy().to_string()]))
    );
}
