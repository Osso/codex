use serde_json::json;

use super::HostrunSession;

#[test]
fn cli_command_builder_spawn_returns_command_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("cli.sleep('1').stdout.text().spawn();")
        .expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.id, "cli.sleep:sleep 1");
    assert_eq!(approval.tool, "cli.sleep");
    assert_eq!(approval.summary, "Spawn sleep 1 (stdout text)");
    assert_eq!(
        approval.args,
        json!({
            "program": "sleep",
            "args": ["1"],
            "stdout": { "type": "text" },
            "action": "spawn"
        })
    );
}
