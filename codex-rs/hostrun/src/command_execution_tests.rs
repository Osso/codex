use std::fs;

use serde_json::json;

use super::HostrunSession;

#[test]
fn approved_cli_command_captures_stdout_text() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session
        .eval("cli.printf('hello').stdout.text().run();")
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "program": "printf",
            "args": ["hello"],
            "exitCode": 0,
            "success": true,
            "stdout": "hello"
        }))
    );
}

#[test]
fn approved_cli_command_pipes_structured_stdin_to_process() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session
        .eval("cli.cat().stdin.lines(['alpha', 'beta']).stdout.lines().run();")
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "program": "cat",
            "args": [],
            "exitCode": 0,
            "success": true,
            "stdout": ["alpha", "beta"]
        }))
    );
}

#[test]
fn approved_cli_command_redirects_stdout_to_file() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let output = dir.path().join("stdout.txt");
    let output_text = output.to_string_lossy().to_string();
    let code = format!(
        "cli.printf('saved').stdout.toFile({}).run();",
        json!(output_text)
    );

    let result = session.eval(&code).expect("eval");

    assert_eq!(fs::read_to_string(&output).expect("stdout file"), "saved");
    assert_eq!(
        result.value,
        Some(json!({
            "program": "printf",
            "args": ["saved"],
            "exitCode": 0,
            "success": true,
            "stdout": {
                "path": output_text,
                "bytes": 5
            }
        }))
    );
}
