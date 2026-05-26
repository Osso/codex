use std::fs;

use serde_json::json;

use super::HostrunSession;

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
