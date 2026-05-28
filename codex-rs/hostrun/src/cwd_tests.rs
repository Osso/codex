use std::fs;

use serde_json::json;

use super::HostrunSession;

#[test]
fn host_cd_updates_session_cwd_for_relative_fs_and_cli_calls() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("probe.txt"), "from cwd\n").expect("write probe");
    let session = HostrunSession::new_auto_approve_with_cwd(dir.path()).expect("session");

    let result = session
        .eval(
            r#"
            ({
              cwd: host.cwd(),
              read: fs.read("probe.txt"),
              pwd: cli.pwd().text().trim()
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "cwd": dir.path().canonicalize().expect("canonical cwd"),
            "read": "from cwd\n",
            "pwd": dir.path().canonicalize().expect("canonical pwd")
        }))
    );
}

#[test]
fn host_cd_canonicalizes_relative_paths_and_persists_across_evals() {
    let dir = tempfile::tempdir().expect("tempdir");
    let nested = dir.path().join("nested");
    fs::create_dir(&nested).expect("create nested");
    fs::write(nested.join("probe.txt"), "nested cwd\n").expect("write probe");
    let session = HostrunSession::new_auto_approve_with_cwd(dir.path()).expect("session");

    session.eval(r#"host.cd("nested");"#).expect("cd");
    let result = session
        .eval(r#"[host.cwd(), fs.read("probe.txt")]"#)
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!([
            nested.canonicalize().expect("canonical nested"),
            "nested cwd\n"
        ]))
    );
}
