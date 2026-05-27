use serde_json::json;

use super::HostrunSession;

#[test]
fn cli_program_proxy_returns_lazy_command_builder() {
    let session = HostrunSession::new().expect("session");

    let result = session.eval("cli.dmidecode();").expect("builder");

    assert_eq!(result.result_type, "completed");
    assert_eq!(
        result.value,
        Some(json!({
            "program": "dmidecode",
            "args": []
        }))
    );
}

#[test]
fn cli_command_builder_run_returns_command_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session.eval("cli.dmidecode().run();").expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    assert_dmidecode_approval(result.approval.expect("approval"));
}

#[test]
fn run_program_proxy_executes_without_capture() {
    let session = HostrunSession::new().expect("session");

    let result = session.eval("run.dmidecode();").expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    assert_dmidecode_approval(result.approval.expect("approval"));
}

fn assert_dmidecode_approval(approval: super::HostrunApprovalRequest) {
    assert_eq!(approval.id, "cli.dmidecode:dmidecode");
    assert_eq!(approval.tool, "cli.dmidecode");
    assert_eq!(approval.summary, "Run dmidecode");
    assert_eq!(
        approval.args,
        json!({
            "program": "dmidecode",
            "args": []
        })
    );
}

#[test]
fn sqlite_query_wrapper_builds_json_sqlite_command() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("sqlite.query('/tmp/app.db', 'select * from users').stdout.json();")
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "sqlite3",
            "args": ["-json", "/tmp/app.db", "select * from users"],
            "stdout": { "type": "text" }
        })
    );
}

#[test]
fn kubectl_get_wrapper_builds_json_get_command() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "kubectl.get('pods', { namespace: 'default', allNamespaces: true })
                .stdout.json();",
        )
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "kubectl",
            "args": ["get", "pods", "--namespace", "default", "--all-namespaces", "-o", "json"],
            "stdout": { "type": "text" }
        })
    );
}
