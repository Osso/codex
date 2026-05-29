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

#[test]
fn tools_file_replace_replaces_exact_text_once() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("replace.txt");
    fs::write(&path, "alpha\nold\nomega\n").expect("write input");

    let result = session
        .eval(&format!(
            "tools.file.replace({}, {{ from: 'old', to: 'new' }});",
            json!(path)
        ))
        .expect("replace");

    assert_eq!(
        fs::read_to_string(&path).expect("updated"),
        "alpha\nnew\nomega\n"
    );
    assert_eq!(result.value.expect("value")["replacements"], json!(1));
}

#[test]
fn tools_file_replace_rejects_ambiguous_matches_by_default() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ambiguous.txt");
    fs::write(&path, "old\nold\n").expect("write input");

    session
        .eval(&format!(
            "tools.file.replace({}, {{ from: 'old', to: 'new' }});",
            json!(path)
        ))
        .expect_err("ambiguous replacement should fail");
}

#[test]
fn tools_file_replace_supports_all_and_occurrence() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("all.txt");
    fs::write(&path, "old\nold\nold\n").expect("write input");

    let all = session
        .eval(&format!(
            "tools.file.replace({}, {{ from: 'old', to: 'new', all: true }});",
            json!(path)
        ))
        .expect("replace all");
    assert_eq!(all.value.expect("value")["replacements"], json!(3));
    assert_eq!(
        fs::read_to_string(&path).expect("all updated"),
        "new\nnew\nnew\n"
    );

    fs::write(&path, "old\nold\nold\n").expect("reset input");
    session
        .eval(&format!(
            "tools.file.replace({}, {{ from: 'old', to: 'new', occurrence: 2 }});",
            json!(path)
        ))
        .expect("replace occurrence");
    assert_eq!(
        fs::read_to_string(&path).expect("occurrence updated"),
        "old\nnew\nold\n"
    );
}

#[test]
fn tools_file_patch_applies_unified_diff() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("patch.txt");
    fs::write(&path, "alpha\nold\nomega\n").expect("write input");
    let diff = format!(
        "--- a/{name}\n+++ b/{name}\n@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n",
        name = path.to_string_lossy()
    );

    let result = session
        .eval(&format!("tools.file.patch({});", json!(diff)))
        .expect("patch");

    assert_eq!(
        fs::read_to_string(&path).expect("updated"),
        "alpha\nnew\nomega\n"
    );
    assert_eq!(result.value.expect("value")[0]["hunks"], json!(1));
}

#[test]
fn tools_file_patch_supports_explicit_path_hunks() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("explicit.txt");
    fs::write(&path, "one\ntwo\nthree\n").expect("write input");
    let diff = "@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n";

    session
        .eval(&format!(
            "tools.file.patch({}, {});",
            json!(path),
            json!(diff)
        ))
        .expect("patch");

    assert_eq!(
        fs::read_to_string(&path).expect("updated"),
        "one\nTWO\nthree\n"
    );
}

#[test]
fn tools_file_patch_can_create_new_files() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("created.txt");
    let diff = format!(
        "--- /dev/null\n+++ b/{name}\n@@ -0,0 +1,2 @@\n+first\n+second\n",
        name = path.to_string_lossy()
    );

    let result = session
        .eval(&format!("tools.file.patch({});", json!(diff)))
        .expect("patch");

    assert_eq!(fs::read_to_string(&path).expect("created"), "first\nsecond");
    assert_eq!(result.value.expect("value")[0]["hunks"], json!(1));
}
