use serde_json::json;

use super::HostrunSession;

#[test]
fn sqlite_query_wrapper_builds_json_sqlite_command() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval("sqlite.query('/tmp/app.db', 'select * from users').stdout.json().run();")
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
                .stdout.json()
                .run();",
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
