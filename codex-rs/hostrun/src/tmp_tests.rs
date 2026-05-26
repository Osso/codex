use serde_json::json;

use super::HostrunSession;

#[test]
fn tmp_file_handle_has_deterministic_path_and_write_approval() {
    let session = HostrunSession::new().expect("session");

    let created = session
        .eval("ctx.probe = tmp.file('probe', { suffix: '.txt' }); ctx.probe.toJSON();")
        .expect("create temp handle");
    assert_eq!(
        created.value,
        Some(json!({ "kind": "file", "path": "/tmp/hostrun-probe-1.txt" }))
    );

    let result = session
        .eval("ctx.probe.write('hello temp');")
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "fs.write");
    assert_eq!(
        approval.args,
        json!({ "path": "/tmp/hostrun-probe-1.txt", "content": "hello temp" })
    );
}

#[test]
fn tmp_file_structured_writes_and_cleanup_use_file_path() {
    let session = HostrunSession::new().expect("session");
    session
        .eval("ctx.data = tmp.file('data', { suffix: '.json' });")
        .expect("create temp handle");

    let write = session
        .eval("ctx.data.writeJson({ ok: true }, 0);")
        .expect("approval");
    assert_eq!(
        write.approval.expect("write approval").args,
        json!({ "path": "/tmp/hostrun-data-1.json", "content": "{\"ok\":true}\n" })
    );

    let cleanup = session.eval("ctx.data.cleanup();").expect("approval");
    let approval = cleanup.approval.expect("cleanup approval");
    assert_eq!(approval.tool, "fs.remove");
    assert_eq!(approval.args, json!({ "path": "/tmp/hostrun-data-1.json" }));
}

#[test]
fn tmp_dir_handle_has_path_and_cleanup_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "const dir = tmp.dir('work area');
             ({ text: String(dir), json: dir.toJSON() });",
        )
        .expect("eval");
    assert_eq!(
        result.value,
        Some(json!({
            "text": "/tmp/hostrun-work-area-1",
            "json": { "kind": "dir", "path": "/tmp/hostrun-work-area-1" }
        }))
    );

    session
        .eval("ctx.dir = tmp.dir('cleanup');")
        .expect("create dir");
    let cleanup = session.eval("ctx.dir.cleanup();").expect("approval");
    assert_eq!(
        cleanup.approval.expect("cleanup approval").args,
        json!({ "path": "/tmp/hostrun-cleanup-2" })
    );
}

#[test]
fn approved_session_removes_tracked_temp_files_on_drop() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let created = session
        .eval(
            "ctx.probe = tmp.file('drop-cleanup', { suffix: '.txt' });
             ctx.probe.write('cleanup');
             ctx.probe.path;",
        )
        .expect("create temp file");
    let path = created
        .value
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .expect("temp path");
    assert!(std::path::Path::new(&path).exists());

    drop(session);

    assert!(!std::path::Path::new(&path).exists());
}

#[test]
fn approved_session_removes_tracked_temp_files_after_eval_error() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let path = session
        .eval(
            "ctx.failed = tmp.file('drop-failure', { suffix: '.txt' });
             ctx.failed.write('cleanup');
             ctx.failed.path;",
        )
        .expect("create temp file")
        .value
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .expect("temp path");
    session
        .eval("throw new Error('after temp creation');")
        .expect_err("eval error");

    drop(session);

    assert!(!std::path::Path::new(&path).exists());
}
