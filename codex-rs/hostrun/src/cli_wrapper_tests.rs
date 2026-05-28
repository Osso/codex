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

#[test]
fn sudo_program_proxy_uses_sudo_binary_literally() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("cli.sudo('dmidecode', '-t', 'system').run();")
        .expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.id, "cli.sudo:sudo dmidecode -t system");
    assert_eq!(approval.tool, "cli.sudo");
    assert_eq!(approval.summary, "Run sudo dmidecode -t system");
    assert_eq!(
        approval.args,
        json!({
            "program": "sudo",
            "args": ["dmidecode", "-t", "system"]
        })
    );
}

#[test]
fn tools_sudo_uses_authsudo() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.sudo(cli.dmidecode('-t', 'system')).run();")
        .expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.id, "cli.authsudo:authsudo dmidecode -t system");
    assert_eq!(approval.tool, "cli.authsudo");
    assert_eq!(approval.summary, "Run authsudo dmidecode -t system");
    assert_eq!(
        approval.args,
        json!({
            "program": "authsudo",
            "args": ["dmidecode", "-t", "system"]
        })
    );
}

#[test]
fn tools_sudo_preserves_command_builder_io() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.sudo(cli.dmidecode('-t', 'system').stdout.capture()).run();")
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "authsudo",
            "args": ["dmidecode", "-t", "system"],
            "stdout": { "type": "capture" }
        })
    );
}

#[test]
fn run_proxy_string_call_explains_correct_api() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("run('dmidecode -t system')")
        .expect("run as a string call should explain the proxy API");

    let value = result.value.expect("explanation");
    assert_eq!(value["ok"], json!(false));
    assert!(
        value["use"]
            .as_array()
            .unwrap()
            .contains(&json!("run.dmidecode('-t', 'system')"))
    );
    assert!(value["use"].as_array().unwrap().contains(&json!(
        "tools.sudo(cli.dmidecode('-t', 'system')).run() for privileged commands"
    )));
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
