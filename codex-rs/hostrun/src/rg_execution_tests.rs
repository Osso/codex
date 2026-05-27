use std::fs;

use serde_json::json;

use super::HostrunSession;

#[test]
fn approved_rg_files_returns_matching_paths() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("one.txt"), "needle\n").expect("write file");
    fs::write(dir.path().join("two.txt"), "nothing\n").expect("write file");
    let root = dir.path().to_string_lossy().to_string();

    let result = session
        .eval(&format!("rg.files('needle', {});", json!(root)))
        .expect("rg files");

    let files = result.value.expect("files");
    assert_eq!(files.as_array().expect("array").len(), 1);
    assert!(files[0].as_str().expect("path").ends_with("one.txt"));
}

#[test]
fn approved_rg_matches_returns_structured_match_objects() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("main.rs");
    fs::write(&path, "fn main() {\n    let needle = true;\n}\n").expect("write file");
    let root = dir.path().to_string_lossy().to_string();

    let result = session
        .eval(&format!("rg.matches('needle', {});", json!(root)))
        .expect("rg matches");

    let matches = result.value.expect("matches");
    assert_eq!(
        matches,
        json!([{
            "path": path.to_string_lossy(),
            "lineNumber": 2,
            "line": "    let needle = true;\n",
            "submatches": [{
                "text": "needle",
                "start": 8,
                "end": 14
            }]
        }])
    );
}
