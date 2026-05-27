use serde_json::json;

use super::HostrunSession;

#[test]
fn field_template_helpers_report_invalid_transform_errors() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            try {
              ["alpha beta"].fields().format("{1|unknown}");
            } catch (error) {
              error.message;
            }
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!("unknown Hostrun field transform: unknown"))
    );
}

#[test]
fn field_template_helpers_define_missing_field_and_literal_behavior() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            const table = ["alpha beta"].fields();
            ({
              missingField: table.format("{3}"),
              malformedTemplate: table.format("{1")
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "missingField": [""],
            "malformedTemplate": ["{1"]
        }))
    );
}
