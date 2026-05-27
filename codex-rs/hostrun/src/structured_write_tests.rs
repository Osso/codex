use serde_json::json;

use super::HostrunSession;

#[test]
fn fs_write_json_serializes_pretty_json_before_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("fs.writeJson('/tmp/data.json', { ok: true, items: ['a', 'b'] });")
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "fs.write");
    assert_eq!(
        approval.args,
        json!({
            "path": "/tmp/data.json",
            "content": "{\n  \"ok\": true,\n  \"items\": [\n    \"a\",\n    \"b\"\n  ]\n}\n"
        })
    );
}

#[test]
fn fs_write_yaml_serializes_nested_values_before_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("fs.writeYaml('/tmp/data.yaml', { name: 'alpha', ports: [80, 443] });")
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "fs.write");
    assert_eq!(
        approval.args,
        json!({
            "path": "/tmp/data.yaml",
            "content": "name: \"alpha\"\nports:\n  - 80\n  - 443\n"
        })
    );
}

#[test]
fn fs_write_toml_serializes_flat_values_before_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("fs.writeToml('/tmp/data.toml', { ok: true, count: 2, name: 'alpha' });")
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "fs.write");
    assert_eq!(
        approval.args,
        json!({
            "path": "/tmp/data.toml",
            "content": "ok = true\ncount = 2\nname = \"alpha\"\n"
        })
    );
}

#[test]
fn fs_write_csv_quotes_cells_before_approval() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            "fs.writeCsv('/tmp/data.csv', [
                ['name', 'note'],
                ['alpha', 'hello, world'],
                ['beta', 'uses \"quotes\"']
            ]);",
        )
        .expect("approval");

    let approval = result.approval.expect("approval");
    assert_eq!(approval.tool, "fs.write");
    assert_eq!(
        approval.args,
        json!({
            "path": "/tmp/data.csv",
            "content": "name,note\nalpha,\"hello, world\"\nbeta,\"uses \"\"quotes\"\"\"\n"
        })
    );
}
