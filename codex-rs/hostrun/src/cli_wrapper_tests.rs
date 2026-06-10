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
    assert_eq!(
        approval.summary,
        "Run authsudo dmidecode -t system (stdout text, stderr text)"
    );
    assert_eq!(
        approval.args,
        json!({
            "program": "authsudo",
            "args": ["dmidecode", "-t", "system"],
            "stdout": { "type": "text" },
            "stderr": { "type": "text" }
        })
    );
}

#[test]
fn tools_sudo_captures_stdout_and_stderr_by_default() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.sudo(cli.ls()).run();")
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "authsudo",
            "args": ["ls"],
            "stdout": { "type": "text" },
            "stderr": { "type": "text" }
        })
    );
}

#[test]
fn tools_sudo_preserves_command_builder_io_overrides() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.sudo(cli.dmidecode('-t', 'system').stdout.capture()).run();")
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "authsudo",
            "args": ["dmidecode", "-t", "system"],
            "stdout": { "type": "capture" },
            "stderr": { "type": "text" }
        })
    );
}

#[test]
fn command_builder_env_is_redacted_in_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("cli.printenv('TOKEN').env('TOKEN', 'plain').stdout.text();")
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(
        approval.summary,
        "Run printenv TOKEN (env TOKEN, stdout text)"
    );
    assert_eq!(
        approval.args,
        json!({
            "program": "printenv",
            "args": ["TOKEN"],
            "env": { "TOKEN": "[redacted]" },
            "stdout": { "type": "text" }
        })
    );
}

#[test]
fn tools_ssh_plain_password_uses_sshpass_env() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "tools.ssh({
              host: 'router',
              user: 'root',
              password: 'none',
              passwordMode: 'plain'
            }).run(cli.echo('hello'));",
        )
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "cli.sshpass");
    assert_eq!(
        approval.summary,
        "Run sshpass -e ssh -o ControlMaster=auto -o ControlPath=~/.ssh/hostrun-%C -o ControlPersist=120s root@router 'echo' 'hello' (env SSHPASS, stdout text, stderr text)"
    );
    assert_eq!(
        approval.args,
        json!({
            "program": "sshpass",
            "args": [
                "-e",
                "ssh",
                "-o",
                "ControlMaster=auto",
                "-o",
                "ControlPath=~/.ssh/hostrun-%C",
                "-o",
                "ControlPersist=120s",
                "root@router",
                "'echo' 'hello'"
            ],
            "env": { "SSHPASS": "[redacted]" },
            "stdout": { "type": "text" },
            "stderr": { "type": "text" }
        })
    );
}

#[test]
fn tools_ssh_rejects_password_without_plain_mode() {
    let session = HostrunSession::new().expect("session");

    session
        .eval("tools.ssh({ host: 'router', password: 'none' }).run(cli.hostname());")
        .expect_err("plain password mode should be explicit");
}

#[test]
fn tools_ssh_cli_returns_lazy_builder_with_persistent_defaults() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.ssh({ host: 'router', port: 2222 }).cli(cli.hostname()).text();")
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "ssh",
            "args": [
                "-p",
                "2222",
                "-o",
                "BatchMode=yes",
                "-o",
                "ControlMaster=auto",
                "-o",
                "ControlPath=~/.ssh/hostrun-%C",
                "-o",
                "ControlPersist=120s",
                "router",
                "'hostname'"
            ],
            "stdout": { "type": "text" }
        })
    );
}

#[test]
fn tools_powershell_composes_with_ssh_using_encoded_command() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"tools.ssh({ host: 'desktop' })
              .cli(tools.powershell("Test-Path 'C:\\World of Warcraft\\_retail_\\Interface\\AddOns'"))
              .text();"#,
        )
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "ssh",
            "args": [
                "-o",
                "BatchMode=yes",
                "-o",
                "ControlMaster=auto",
                "-o",
                "ControlPath=~/.ssh/hostrun-%C",
                "-o",
                "ControlPersist=120s",
                "desktop",
                "powershell -NoProfile -EncodedCommand VABlAHMAdAAtAFAAYQB0AGgAIAAnAEMAOgBcAFcAbwByAGwAZAAgAG8AZgAgAFcAYQByAGMAcgBhAGYAdABcAF8AcgBlAHQAYQBpAGwAXwBcAEkAbgB0AGUAcgBmAGEAYwBlAFwAQQBkAGQATwBuAHMAJwA="
            ],
            "stdout": { "type": "text" }
        })
    );
}

#[test]
fn tools_ssh_opt_outs_disable_multiplex_and_batch_mode() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "tools.ssh({ host: 'router', multiplex: false, batchMode: false })
              .cli(cli.hostname()).text();",
        )
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "ssh",
            "args": ["router", "'hostname'"],
            "stdout": { "type": "text" }
        })
    );
}

#[test]
fn tools_ssh_explicit_options_override_matching_defaults() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "tools.ssh({ host: 'router', options: ['ControlMaster=no', 'BatchMode no'] })
              .cli(cli.hostname()).text();",
        )
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "ssh",
            "args": [
                "-o",
                "ControlMaster=no",
                "-o",
                "BatchMode no",
                "-o",
                "ControlPath=~/.ssh/hostrun-%C",
                "-o",
                "ControlPersist=120s",
                "router",
                "'hostname'"
            ],
            "stdout": { "type": "text" }
        })
    );
}

#[test]
fn browser_open_builds_browser_cli_command() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.browser.open('https://example.com').run();")
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(
        approval.id,
        "cli.browser-cli:browser-cli open https://example.com"
    );
    assert_eq!(approval.tool, "cli.browser-cli");
    assert_eq!(
        approval.args,
        json!({
            "program": "browser-cli",
            "args": ["open", "https://example.com"]
        })
    );
}

#[test]
fn browser_get_helpers_capture_text() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.browser.get('title').text();")
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "browser-cli",
            "args": ["get", "title"],
            "stdout": { "type": "text" }
        })
    );
}

#[test]
fn browser_snapshot_and_screenshot_build_expected_flags() {
    let session = HostrunSession::new().expect("session");

    let snapshot = session
        .eval("tools.browser.snapshot({ mini: true, interactive: true, depth: 4 }).text();")
        .expect("snapshot approval");

    assert_eq!(
        snapshot.approval.expect("approval").args,
        json!({
            "program": "browser-cli",
            "args": ["snapshot", "--mini", "--interactive", "--depth", "4"],
            "stdout": { "type": "text" }
        })
    );

    let screenshot = session
        .eval("tools.browser.screenshot('/tmp/page.jpg', { full: true }).run();")
        .expect("screenshot approval");

    assert_eq!(
        screenshot.approval.expect("approval").args,
        json!({
            "program": "browser-cli",
            "args": ["screenshot", "--full", "/tmp/page.jpg"]
        })
    );
}

#[test]
fn browser_runtime_helpers_capture_json() {
    let session = HostrunSession::new().expect("session");

    let console = session
        .eval("tools.browser.console({ reload: true, waitMs: 3000 }).json();")
        .expect("console approval");

    assert_eq!(
        console.approval.expect("approval").args,
        json!({
            "program": "browser-cli",
            "args": ["--json", "runtime", "console", "--reload", "--wait-ms", "3000"],
            "stdout": { "type": "text" }
        })
    );

    let exceptions = session
        .eval("tools.browser.exceptions({ reload: true }).json();")
        .expect("exceptions approval");

    assert_eq!(
        exceptions.approval.expect("approval").args,
        json!({
            "program": "browser-cli",
            "args": ["--json", "runtime", "exceptions", "--reload"],
            "stdout": { "type": "text" }
        })
    );
}

#[test]
fn browser_tabs_build_nested_commands() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.browser.tabs.switch(2).run();")
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "browser-cli",
            "args": ["tabs", "switch", "2"]
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
