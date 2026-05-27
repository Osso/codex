use serde_json::json;

use super::HostrunSession;

#[test]
fn date_helpers_parse_format_and_humanize_utc_dates() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            ({
              parsed: date.parse("2026-05-27T12:34:56Z").toISOString(),
              formatted: date.format("2026-05-27T12:34:56Z", "YYYY/MM/DD HH:mm:ss Z"),
              humanPast: date.humanize("2026-05-27T12:04:56Z", "2026-05-27T12:34:56Z"),
              humanFuture: date.humanize("2026-05-28T12:34:56Z", "2026-05-27T12:34:56Z")
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "parsed": "2026-05-27T12:34:56.000Z",
            "formatted": "2026/05/27 12:34:56 Z",
            "humanPast": "30 minutes ago",
            "humanFuture": "1 day from now"
        }))
    );
}
