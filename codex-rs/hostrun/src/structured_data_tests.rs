use serde_json::json;

use super::HostrunSession;

#[test]
fn string_helpers_parse_csv_tsv_and_jsonl() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            ({
              csv: 'name,note\nalpha,"hello, world"\nbeta,"uses ""quotes"""\n'.csv(),
              tsv: 'name\tnote\nalpha\thello\\tthere\nbeta\tline\\nbreak\n'.tsv(),
              jsonl: '{"name":"alpha"}\n{"name":"beta","ok":true}\n'.jsonl(),
              yaml: 'name: alpha\nports:\n  - 80\n  - 443\nactive: true\n'.yaml()
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "csv": [
                ["name", "note"],
                ["alpha", "hello, world"],
                ["beta", "uses \"quotes\""]
            ],
            "tsv": [
                ["name", "note"],
                ["alpha", "hello\tthere"],
                ["beta", "line\nbreak"]
            ],
            "jsonl": [
                { "name": "alpha" },
                { "name": "beta", "ok": true }
            ],
            "yaml": {
                "name": "alpha",
                "ports": [80, 443],
                "active": true
            }
        }))
    );
}

#[test]
fn fs_write_tsv_and_json_lines_serialize_before_approval() {
    let session = HostrunSession::new().expect("session");

    let tsv = session
        .eval("fs.writeTsv('/tmp/data.tsv', [['name', 'note'], ['alpha', 'hello\\tthere']]);")
        .expect("approval");
    assert_eq!(
        tsv.approval.expect("tsv approval").args,
        json!({
            "path": "/tmp/data.tsv",
            "content": "name\tnote\nalpha\thello\\tthere\n"
        })
    );

    let jsonl = session
        .eval("fs.writeJsonLines('/tmp/data.jsonl', [{ name: 'alpha' }, { ok: true }]);")
        .expect("approval");
    assert_eq!(
        jsonl.approval.expect("jsonl approval").args,
        json!({
            "path": "/tmp/data.jsonl",
            "content": "{\"name\":\"alpha\"}\n{\"ok\":true}\n"
        })
    );
}

#[test]
fn command_builder_preserves_structured_stdin_sources() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            cli.cat()
              .stdin.csv([['name', 'note'], ['alpha', 'hello']])
              .stdout.capture()
              .run();
            "#,
        )
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "program": "cat",
            "args": [],
            "stdin": {
                "type": "csv",
                "rows": [["name", "note"], ["alpha", "hello"]]
            },
            "stdout": { "type": "capture" }
        })
    );
}

#[test]
fn tmp_file_forwards_tsv_and_jsonl_writes_to_file_path() {
    let session = HostrunSession::new().expect("session");
    session
        .eval("ctx.out = tmp.file('structured', { suffix: '.jsonl' });")
        .expect("create temp handle");

    let result = session
        .eval("ctx.out.writeJsonl([{ ok: true }]);")
        .expect("approval");

    assert_eq!(
        result.approval.expect("approval").args,
        json!({
            "path": "/tmp/hostrun-structured-1.jsonl",
            "content": "{\"ok\":true}\n"
        })
    );
}

#[test]
fn conversion_helpers_serialize_json_yaml_csv_tsv_and_jsonl() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            const rows = [['name', 'note'], ['alpha', 'hello, world']];
            ({
              json: ({ ok: true, count: 2 }).toJson(),
              yaml: ({ ok: true, count: 2 }).toYaml(),
              csv: rows.toCsv(),
              tsv: rows.toTsv(),
              jsonl: [{ name: 'alpha' }, { ok: true }].toJsonl()
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "json": "{\"ok\":true,\"count\":2}",
            "yaml": "ok: true\ncount: 2\n",
            "csv": "name,note\nalpha,\"hello, world\"\n",
            "tsv": "name\tnote\nalpha\thello, world\n",
            "jsonl": "{\"name\":\"alpha\"}\n{\"ok\":true}\n"
        }))
    );
}

#[test]
fn structured_parsers_report_malformed_input_errors() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            const messages = [];
            for (const parse of [
              () => '{bad json'.json(),
              () => '{"ok":true}\nnot-json\n'.jsonl()
            ]) {
              try {
                parse();
              } catch (error) {
                messages.push(error.message);
              }
            }
            messages;
            "#,
        )
        .expect("eval");

    let messages = result.value.expect("messages");
    assert!(
        messages[0].as_str().expect("json error").contains("JSON"),
        "unexpected JSON parser error: {messages}"
    );
    assert!(
        !messages[1].as_str().expect("jsonl error").is_empty(),
        "unexpected JSONL parser error: {messages}"
    );
}
