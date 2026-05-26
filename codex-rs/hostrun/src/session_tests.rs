use serde_json::json;

use super::HostrunSession;
use super::HostrunSessionStore;

#[test]
fn keeps_live_ctx_objects_across_evaluations() {
    let session = HostrunSession::new().expect("session");
    session
        .eval("ctx.files = ['a.txt', 'probe.txt'];")
        .expect("set ctx");

    let result = session
        .eval("ctx.probes = ctx.files.containing('probe'); ctx.probes.length;")
        .expect("filter ctx");

    assert_eq!(result.value, Some(json!(1)));
}

#[test]
fn keeps_ctx_alive_after_normal_exception() {
    let session = HostrunSession::new().expect("session");
    session
        .eval("ctx.counter = { value: 41 };")
        .expect("set ctx");
    session
        .eval("throw new Error('boom');")
        .expect_err("normal exception should be returned");

    let result = session
        .eval("ctx.counter.value += 1;")
        .expect("increment ctx");

    assert_eq!(result.value, Some(json!(42)));
}

#[test]
fn returns_built_in_fs_write_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("tools.fs.write({ path: '/tmp/hostrun.txt', content: 'hello' });")
        .expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.id, "fs.write:/tmp/hostrun.txt");
    assert_eq!(approval.tool, "fs.write");
    assert_eq!(approval.summary, "Write 5 bytes to /tmp/hostrun.txt");
    assert_eq!(
        approval.args,
        json!({ "path": "/tmp/hostrun.txt", "content": "hello" })
    );
}

#[test]
fn public_fs_write_returns_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("fs.write('/tmp/hostrun.txt', 'hello');")
        .expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.id, "fs.write:/tmp/hostrun.txt");
    assert_eq!(approval.tool, "fs.write");
    assert_eq!(approval.summary, "Write 5 bytes to /tmp/hostrun.txt");
    assert_eq!(
        approval.args,
        json!({ "path": "/tmp/hostrun.txt", "content": "hello" })
    );
}

#[test]
fn public_fs_read_returns_approval() {
    assert_fs_path_approval(
        "fs.read('/tmp/hostrun.txt');",
        "fs.read",
        "Read /tmp/hostrun.txt",
    );
}

#[test]
fn public_fs_exists_returns_approval() {
    assert_fs_path_approval(
        "fs.exists('/tmp/hostrun.txt');",
        "fs.exists",
        "Check existence of /tmp/hostrun.txt",
    );
}

#[test]
fn public_fs_remove_returns_approval() {
    assert_fs_path_approval(
        "fs.remove('/tmp/hostrun.txt');",
        "fs.remove",
        "Remove /tmp/hostrun.txt",
    );
}

#[test]
fn public_rclone_deletefile_returns_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("rclone.deletefile('spaces:bucket/probe.txt');")
        .expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.id, "rclone.deletefile:spaces:bucket/probe.txt");
    assert_eq!(approval.tool, "rclone.deletefile");
    assert_eq!(approval.summary, "Delete spaces:bucket/probe.txt");
    assert_eq!(
        approval.args,
        json!({ "target": "spaces:bucket/probe.txt" })
    );
}

fn assert_fs_path_approval(code: &str, tool: &str, summary: &str) {
    let session = HostrunSession::new().expect("session");

    let result = session.eval(code).expect("approval");

    assert_eq!(result.result_type, "needs_approval");
    let approval = result.approval.expect("approval");
    assert_eq!(approval.id, format!("{tool}:/tmp/hostrun.txt"));
    assert_eq!(approval.tool, tool);
    assert_eq!(approval.summary, summary);
    assert_eq!(approval.args, json!({ "path": "/tmp/hostrun.txt" }));
}

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
    let approval = result.approval.expect("approval");
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
fn cli_program_proxy_preserves_arguments() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("cli.rg('needle', 'src', { '--json': true }).run();")
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "cli.rg");
    assert_eq!(
        approval.args,
        json!({
            "program": "rg",
            "args": ["needle", "src", { "--json": true }]
        })
    );
}

#[test]
fn cli_command_builder_includes_io_metadata_in_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "cli.rg('needle', 'src')
              .stdout.toFile('/tmp/matches.txt')
              .stderr.toStdout()
              .stdin.text('input')
              .run();",
        )
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "cli.rg");
    assert_eq!(
        approval.args,
        json!({
            "program": "rg",
            "args": ["needle", "src"],
            "stdout": { "type": "file", "path": "/tmp/matches.txt" },
            "stderr": { "type": "stdout" },
            "stdin": { "type": "text", "text": "input" }
        })
    );
}

#[test]
fn cli_command_builder_can_pipe_from_named_stdout_handle() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "const source = cli.rclone('cat', 'spaces:bucket/index.txt');
             cli.cat().stdin(source.stdout).combined.capture().run();",
        )
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "cli.cat");
    assert_eq!(
        approval.args,
        json!({
            "program": "cat",
            "args": [],
            "stdin": {
                "type": "stream",
                "source": {
                    "stream": "stdout",
                    "command": {
                        "program": "rclone",
                        "args": ["cat", "spaces:bucket/index.txt"]
                    }
                }
            },
            "combined": { "type": "capture" }
        })
    );
}

#[test]
fn rclone_lsf_wrapper_builds_rclone_command() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("rclone.lsf('spaces:bucket', { recursive: true }).stdout.lines().run();")
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "cli.rclone");
    assert_eq!(
        approval.args,
        json!({
            "program": "rclone",
            "args": ["lsf", "spaces:bucket", "--recursive"],
            "stdout": { "type": "lines" }
        })
    );
}

#[test]
fn fd_files_wrapper_builds_fdfind_command() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("fd.files('/repo', { extension: 'rs', hidden: true, exclude: ['target'] }).run();")
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "cli.fdfind");
    assert_eq!(
        approval.args,
        json!({
            "program": "fdfind",
            "args": [
                ".",
                "--type",
                "file",
                "--extension",
                "rs",
                "--hidden",
                "--exclude",
                "target",
                "/repo"
            ]
        })
    );
}

#[test]
fn rg_search_wrapper_builds_rg_command() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "rg.search('needle', 'src', {
                fixed: true,
                ignoreCase: true,
                json: true,
                glob: '*.rs'
            }).run();",
        )
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "cli.rg");
    assert_eq!(
        approval.args,
        json!({
            "program": "rg",
            "args": [
                "--fixed-strings",
                "--ignore-case",
                "--json",
                "--glob",
                "*.rs",
                "needle",
                "src"
            ]
        })
    );
}

#[test]
fn http_get_json_request_includes_redacted_metadata() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "http.get('https://api.example.com/users', {
                query: { q: 'hostrun', limit: 20 },
                headers: {
                    Accept: 'application/json',
                    Authorization: 'Bearer secret',
                    'X-Api-Key': 'secret-key'
                },
                auth: { bearer: 'secret-token' },
                timeout: '10s',
                retries: 2
            }).json();",
        )
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(
        approval.id,
        "http.request:GET:https://api.example.com/users"
    );
    assert_eq!(approval.tool, "http.request");
    assert_eq!(approval.summary, "HTTP GET https://api.example.com/users");
    assert_eq!(
        approval.args,
        json!({
            "method": "GET",
            "url": "https://api.example.com/users",
            "query": { "q": "hostrun", "limit": 20 },
            "headers": {
                "Accept": "application/json",
                "Authorization": "<redacted>",
                "X-Api-Key": "<redacted>"
            },
            "auth": { "bearer": "<redacted>" },
            "timeout": "10s",
            "retries": 2,
            "response": { "type": "json" }
        })
    );
}

#[test]
fn http_post_json_body_can_save_response_to_file() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "http.post('https://api.example.com/users', {
                json: { name: 'Alice' },
                auth: { basic: { username: 'alice', password: 'secret' } }
            }).save('/tmp/user.json');",
        )
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(
        approval.args,
        json!({
            "method": "POST",
            "url": "https://api.example.com/users",
            "json": { "name": "Alice" },
            "auth": {
                "basic": { "username": "alice", "password": "<redacted>" }
            },
            "response": { "type": "file", "path": "/tmp/user.json" }
        })
    );
}

#[test]
fn http_rejects_ambiguous_body_sources() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "try {
                http.post('https://api.example.com/users', {
                    json: {},
                    body: 'raw'
                }).run();
            } catch (error) {
                error.message;
            }",
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!("http request has multiple body sources: json, body"))
    );
}

#[test]
fn http_multipart_file_metadata_is_preserved() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "http.post('https://upload.example.com/files', {
                multipart: {
                    title: 'Probe',
                    upload: {
                        file: '/tmp/probe.txt',
                        filename: 'probe.txt',
                        contentType: 'text/plain'
                    }
                }
            }).text();",
        )
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(
        approval.args,
        json!({
            "method": "POST",
            "url": "https://upload.example.com/files",
            "multipart": {
                "title": "Probe",
                "upload": {
                    "file": "/tmp/probe.txt",
                    "filename": "probe.txt",
                    "contentType": "text/plain"
                }
            },
            "response": { "type": "text" }
        })
    );
}

#[test]
fn captures_console_messages_and_echoes_executed_code() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("console.log('hello', { ok: true }); console.debug('trace'); 42;")
        .expect("eval");

    assert_eq!(
        result.executed,
        "console.log('hello', { ok: true }); console.debug('trace'); 42;"
    );
    assert_eq!(result.value, Some(json!(42)));
    assert_eq!(
        serde_json::to_value(result.console).expect("console serializes"),
        json!([
            { "level": "log", "message": "hello {\"ok\":true}" },
            { "level": "debug", "message": "trace" }
        ])
    );
}

#[test]
fn string_helpers_parse_lines_json_and_json_lines() {
    let session = HostrunSession::new().expect("session");
    let result = session
        .eval(
            r#"
            ({
              lines: "a\r\nb\n".lines(),
              json: '{"ok":true,"n":2}'.json(),
              jsonLines: '{"a":1}\n{"a":2}\n'.jsonLines(),
              lower: "HeLLo".lower(),
              upper: "HeLLo".upper(),
              bytes: "é".bytes(),
              chars: "éx".chars()
            });
            "#,
        )
        .expect("eval");
    assert_eq!(
        result.value,
        Some(json!({
            "lines": ["a", "b", ""],
            "json": { "ok": true, "n": 2 },
            "jsonLines": [{ "a": 1 }, { "a": 2 }],
            "lower": "hello",
            "upper": "HELLO",
            "bytes": 2,
            "chars": ["é", "x"]
        }))
    );
}

#[test]
fn array_helpers_filter_and_transform_strings_without_mutating() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            const items = ["beta.txt", "alpha.rs", "alpha.rs", "src/main.rs"];
            ({
              original: items,
              notContaining: items.notContaining("src"),
              startsWith: items.startsWith("alpha"),
              endsWith: items.endsWith(".rs"),
              matching: items.matching(/alpha|main/),
              notMatching: items.notMatching(/alpha/),
              glob: items.glob("**/*.rs"),
              notGlob: items.notGlob("*.txt"),
              first: items.first(),
              last: items.last(),
              take: items.take(2),
              unique: items.unique(),
              lengths: items.lengths(),
              bytes: ["é", "x"].bytes(),
              lower: ["A", "B"].lower(),
              upper: ["a", "b"].upper(),
              sorted: items.sorted(),
              reversed: items.reversed()
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "original": ["beta.txt", "alpha.rs", "alpha.rs", "src/main.rs"],
            "notContaining": ["beta.txt", "alpha.rs", "alpha.rs"],
            "startsWith": ["alpha.rs", "alpha.rs"],
            "endsWith": ["alpha.rs", "alpha.rs", "src/main.rs"],
            "matching": ["alpha.rs", "alpha.rs", "src/main.rs"],
            "notMatching": ["beta.txt", "src/main.rs"],
            "glob": ["alpha.rs", "alpha.rs", "src/main.rs"],
            "notGlob": ["alpha.rs", "alpha.rs", "src/main.rs"],
            "first": "beta.txt",
            "last": "src/main.rs",
            "take": ["beta.txt", "alpha.rs"],
            "unique": ["beta.txt", "alpha.rs", "src/main.rs"],
            "lengths": [8, 8, 8, 11],
            "bytes": [2, 1],
            "lower": ["a", "b"],
            "upper": ["A", "B"],
            "sorted": ["alpha.rs", "alpha.rs", "beta.txt", "src/main.rs"],
            "reversed": ["src/main.rs", "alpha.rs", "alpha.rs", "beta.txt"]
        }))
    );
}

#[test]
fn fields_helper_groups_counts_uniques_and_sorts_by_selectors() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            const lines = [
              "bob active beta-222",
              "alice active alpha-111",
              "bob inactive beta-333",
              "carol active gamma-444"
            ];
            const table = lines.fields();
            ({
              sortedUsers: table.sortBy(1).format("{1}:{3|substr:0,4}"),
              uniqueUsers: table.uniqueBy(1).format("{1}:{2}"),
              counts: table.countBy("{2|upper}"),
              groups: table.groupBy("{1}-{3|substr:0,4}").map((group) => ({
                key: group.key,
                rows: group.rows
              }))
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "sortedUsers": ["alice:alph", "bob:beta", "bob:beta", "carol:gamm"],
            "uniqueUsers": ["bob:active", "alice:active", "carol:active"],
            "counts": [
                { "key": "ACTIVE", "count": 3 },
                { "key": "INACTIVE", "count": 1 }
            ],
            "groups": [
                {
                    "key": "bob-beta",
                    "rows": [
                        ["bob", "active", "beta-222"],
                        ["bob", "inactive", "beta-333"]
                    ]
                },
                {
                    "key": "alice-alph",
                    "rows": [["alice", "active", "alpha-111"]]
                },
                {
                    "key": "carol-gamm",
                    "rows": [["carol", "active", "gamma-444"]]
                }
            ]
        }))
    );
}

#[test]
fn fields_helper_formats_text_and_object_templates() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            const lines = [
              "alice   active   publisher-upload-12345",
              "bob     active   mangahelpers-99999"
            ];
            ({
              rows: lines.fields().rows(),
              users: lines.fields().field(1),
              text: lines.fields().format("user:{1} prefix:{3|substr:0,7}"),
              objects: lines.fields().format({
                user: "{1|upper}",
                prefix: "{3|substr:0,7}",
                base: "{3|basename}",
                dir: "{3|dirname}"
              })
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "rows": [
                ["alice", "active", "publisher-upload-12345"],
                ["bob", "active", "mangahelpers-99999"]
            ],
            "users": ["alice", "bob"],
            "text": ["user:alice prefix:publish", "user:bob prefix:mangahe"],
            "objects": [
                {
                    "user": "ALICE",
                    "prefix": "publish",
                    "base": "publisher-upload-12345",
                    "dir": "."
                },
                {
                    "user": "BOB",
                    "prefix": "mangahe",
                    "base": "mangahelpers-99999",
                    "dir": "."
                }
            ]
        }))
    );
}

#[test]
fn store_keeps_sessions_separate_and_persistent() {
    let mut store = HostrunSessionStore::new();

    let first = store
        .eval("session-1", "ctx.count = 41; ctx.count;")
        .expect("first");
    let second = store
        .eval("session-1", "ctx.count += 1; ctx.count;")
        .expect("second");
    let other = store
        .eval("session-2", "ctx.count ?? null;")
        .expect("other");

    assert_eq!(first.value, Some(json!(41)));
    assert_eq!(second.value, Some(json!(42)));
    assert_eq!(
        serde_json::to_value(other).expect("other result"),
        json!({
            "type": "completed",
            "executed": "ctx.count ?? null;",
            "value": null
        })
    );
}
