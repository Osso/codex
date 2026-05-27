use std::fs;

use serde_json::json;

use super::HostrunSession;

#[test]
fn approved_cli_command_captures_stdout_text() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session
        .eval("cli.printf('hello').stdout.text();")
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "program": "printf",
            "args": ["hello"],
            "exitCode": 0,
            "success": true,
            "stdout": "hello",
            "stdoutMeta": {
                "bytes": 5,
                "capturedBytes": 5,
                "truncated": false
            }
        }))
    );
}

#[test]
fn command_builder_text_shortcut_executes_stdout_without_run() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session.eval("cli.printf('hello').text();").expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "program": "printf",
            "args": ["hello"],
            "exitCode": 0,
            "success": true,
            "stdout": "hello",
            "stdoutMeta": {
                "bytes": 5,
                "capturedBytes": 5,
                "truncated": false
            }
        }))
    );
}

#[test]
fn approved_cli_command_pipes_structured_stdin_to_process() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session
        .eval("cli.cat().stdin.lines(['alpha', 'beta']).stdout.lines();")
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "program": "cat",
            "args": [],
            "exitCode": 0,
            "success": true,
            "stdout": ["alpha", "beta"],
            "stdoutMeta": {
                "bytes": 11,
                "capturedBytes": 11,
                "truncated": false
            }
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

#[test]
fn approved_cli_command_captures_stderr_and_combined_output() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let stderr = session
        .eval("cli.sh('-c', 'printf err >&2').stderr.text();")
        .expect("stderr eval");
    assert_eq!(
        stderr.value,
        Some(json!({
            "program": "sh",
            "args": ["-c", "printf err >&2"],
            "exitCode": 0,
            "success": true,
            "stderr": "err",
            "stderrMeta": {
                "bytes": 3,
                "capturedBytes": 3,
                "truncated": false
            }
        }))
    );

    let combined = session
        .eval("cli.sh('-c', 'printf out; printf err >&2').combined.capture().run();")
        .expect("combined eval");
    assert_eq!(
        combined.value.expect("combined value")["combined"],
        json!("outerr")
    );
}

#[test]
fn approved_cli_command_can_redirect_stderr_to_stdout() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session
        .eval(
            "cli.sh('-c', 'printf out; printf err >&2')
              .stderr.toStdout()
              .stdout.text();",
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "program": "sh",
            "args": ["-c", "printf out; printf err >&2"],
            "exitCode": 0,
            "success": true,
            "stdout": "outerr",
            "stdoutMeta": {
                "bytes": 6,
                "capturedBytes": 6,
                "truncated": false
            }
        }))
    );
}

#[test]
fn approved_cli_command_complete_captures_stdout_stderr_and_exit_status() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session
        .eval("cli.sh('-c', 'printf out; printf err >&2; exit 7').complete();")
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "program": "sh",
            "args": ["-c", "printf out; printf err >&2; exit 7"],
            "exitCode": 7,
            "success": false,
            "stdout": "out",
            "stdoutMeta": {
                "bytes": 3,
                "capturedBytes": 3,
                "truncated": false
            },
            "stderr": "err",
            "stderrMeta": {
                "bytes": 3,
                "capturedBytes": 3,
                "truncated": false
            }
        }))
    );
}

#[test]
fn approved_cli_command_spawn_returns_handle_and_waits_for_output() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session
        .eval(
            r#"
            const process = cli.printf('spawned').stdout.capture().spawn();
            ({
              handle: {
                id: process.id,
                pidType: typeof process.pid,
                program: process.program,
                args: process.args,
                stdout: process.stdout,
                stderr: process.stderr
              },
              waited: process.wait()
            });
            "#,
        )
        .expect("eval");
    let value = result.value.expect("value");

    assert_eq!(value["handle"]["id"], "process-1");
    assert_eq!(value["handle"]["pidType"], "number");
    assert_eq!(value["handle"]["program"], "printf");
    assert_eq!(value["handle"]["args"], json!(["spawned"]));
    assert_eq!(
        value["handle"]["stdout"],
        json!({ "process": "process-1", "stream": "stdout" })
    );
    assert_eq!(
        value["handle"]["stderr"],
        json!({ "process": "process-1", "stream": "stderr" })
    );
    assert_eq!(
        value["waited"],
        json!({
            "program": "printf",
            "args": ["spawned"],
            "exitCode": 0,
            "success": true,
            "stdout": "spawned",
            "stdoutMeta": {
                "bytes": 7,
                "capturedBytes": 7,
                "truncated": false
            }
        })
    );
}

#[test]
fn approved_cli_command_spawn_can_kill_process_handle() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session
        .eval(
            r#"
            const process = cli.sleep('5').spawn();
            process.kill();
            "#,
        )
        .expect("eval");
    let value = result.value.expect("value");

    assert_eq!(value["id"], "process-1");
    assert_eq!(value["program"], "sleep");
    assert_eq!(value["args"], json!(["5"]));
    assert_eq!(value["success"], false);
    assert_eq!(value["killed"], true);
}

#[test]
fn approved_cli_command_tees_stdout_to_file_and_captures_text() {
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let output = dir.path().join("tee.txt");
    let output_text = output.to_string_lossy().to_string();
    let code = format!(
        "cli.printf('visible').stdout.tee({}).run();",
        json!(output_text)
    );

    let result = session.eval(&code).expect("eval");

    assert_eq!(fs::read_to_string(&output).expect("tee file"), "visible");
    assert_eq!(
        result.value,
        Some(json!({
            "program": "printf",
            "args": ["visible"],
            "exitCode": 0,
            "success": true,
            "stdout": "visible",
            "stdoutFile": {
                "path": output_text,
                "bytes": 7
            },
            "stdoutMeta": {
                "bytes": 7,
                "capturedBytes": 7,
                "truncated": false
            }
        }))
    );
}

#[test]
fn approved_cli_command_bounds_captured_output() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session
        .eval("cli.sh('-c', 'printf %070000d 0').stdout.text();")
        .expect("eval");
    let value = result.value.expect("value");

    assert_eq!(value["stdout"].as_str().expect("stdout").len(), 64 * 1024);
    assert_eq!(
        value["stdoutMeta"],
        json!({
            "bytes": 70000,
            "capturedBytes": 64 * 1024,
            "truncated": true
        })
    );
}

#[test]
fn approved_cli_command_serializes_structured_stdin_sources() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let json_result = session
        .eval("cli.cat().stdin.json({ ok: true }).stdout.text();")
        .expect("json stdin");
    assert_eq!(
        json_result.value.expect("json value")["stdout"],
        "{\"ok\":true}\n"
    );

    let yaml_result = session
        .eval("cli.cat().stdin.yaml({ ok: true }).stdout.text();")
        .expect("yaml stdin");
    assert_eq!(
        yaml_result.value.expect("yaml value")["stdout"],
        "ok: true\n"
    );

    let csv_result = session
        .eval("cli.cat().stdin.csv([['name', 'ok'], ['alpha', true]]).stdout.text();")
        .expect("csv stdin");
    assert_eq!(
        csv_result.value.expect("csv value")["stdout"],
        "name,ok\nalpha,true\n"
    );

    let jsonl_result = session
        .eval("cli.cat().stdin.jsonl([{ name: 'alpha' }, { ok: true }]).stdout.text();")
        .expect("jsonl stdin");
    assert_eq!(
        jsonl_result.value.expect("jsonl value")["stdout"],
        "{\"name\":\"alpha\"}\n{\"ok\":true}\n"
    );
}

#[test]
fn approved_cli_command_parses_structured_stdout() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let json_result = session
        .eval(r#"cli.printf('{"ok":true,"count":2}').stdout.json();"#)
        .expect("json stdout");
    assert_eq!(
        json_result.value.expect("json value")["stdout"],
        json!({ "ok": true, "count": 2 })
    );

    let jsonl_result = session
        .eval(r#"cli.printf('{"name":"alpha"}\n{"name":"beta"}\n').stdout.jsonl();"#)
        .expect("jsonl stdout");
    assert_eq!(
        jsonl_result.value.expect("jsonl value")["stdout"],
        json!([{ "name": "alpha" }, { "name": "beta" }])
    );

    let csv_result = session
        .eval(r#"cli.printf('name,note\nalpha,"hello, world"\n').stdout.csv();"#)
        .expect("csv stdout");
    assert_eq!(
        csv_result.value.expect("csv value")["stdout"],
        json!([["name", "note"], ["alpha", "hello, world"]])
    );

    let tsv_result = session
        .eval(r#"cli.printf('%s', 'name\tnote\nalpha\thello\\tthere\n').stdout.tsv();"#)
        .expect("tsv stdout");
    assert_eq!(
        tsv_result.value.expect("tsv value")["stdout"],
        json!([["name", "note"], ["alpha", "hello\tthere"]])
    );

    let yaml_result = session
        .eval("cli.printf('%s', 'name: alpha\\nactive: true\\n').stdout.yaml();")
        .expect("yaml stdout");
    assert_eq!(
        yaml_result.value.expect("yaml value")["stdout"],
        json!({ "name": "alpha", "active": true })
    );
}

#[test]
fn approved_cli_command_pipes_from_upstream_stdout_and_stderr() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let stdout = session
        .eval(
            "const source = cli.printf('from stdout');
             cli.cat().stdin(source.stdout).stdout.text();",
        )
        .expect("stdout pipe");
    assert_eq!(
        stdout.value.as_ref().expect("stdout value")["stdout"],
        json!("from stdout")
    );
    assert_eq!(
        stdout.value.as_ref().expect("stdout value")["commands"],
        json!([
            {
                "program": "printf",
                "args": ["from stdout"],
                "exitCode": 0,
                "success": true
            },
            {
                "program": "cat",
                "args": [],
                "exitCode": 0,
                "success": true
            }
        ])
    );

    let stderr = session
        .eval(
            "const source = cli.sh('-c', 'printf from-stderr >&2');
             cli.cat().stdin(source.stderr).stdout.text();",
        )
        .expect("stderr pipe");
    assert_eq!(
        stderr.value.expect("stderr value")["stdout"],
        json!("from-stderr")
    );
}

#[test]
fn approved_cli_command_graph_reports_upstream_failures() {
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session
        .eval(
            "const source = cli.sh('-c', 'printf partial; exit 9');
             cli.cat().stdin(source.stdout).stdout.text();",
        )
        .expect("graph eval");
    let value = result.value.expect("value");

    assert_eq!(value["stdout"], json!("partial"));
    assert_eq!(value["success"], json!(false));
    assert_eq!(
        value["commands"],
        json!([
            {
                "program": "sh",
                "args": ["-c", "printf partial; exit 9"],
                "exitCode": 9,
                "success": false
            },
            {
                "program": "cat",
                "args": [],
                "exitCode": 0,
                "success": true
            }
        ])
    );
}
