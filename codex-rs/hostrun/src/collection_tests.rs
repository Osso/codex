use serde_json::json;

use super::HostrunSession;

#[test]
fn collection_shape_helpers_are_non_mutating() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            const values = [1, null, "", 4];
            const rows = [["name", "age"], ["alice", 3], ["bob"]];
            ({
              flattened: [[1], [2, [3]]].flatten(2),
              compact: values.compact(),
              defaults: values.default("missing"),
              wrapped: ["alpha", "beta"].wrap("name"),
              transpose: rows.transpose(),
              enumerate: ["a", "b"].enumerate(),
              empty: [].isEmpty(),
              notEmpty: values.isNotEmpty(),
              originalValues: values,
              originalRows: rows
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "flattened": [1, 2, 3],
            "compact": [1, 4],
            "defaults": [1, "missing", "missing", 4],
            "wrapped": [{ "name": "alpha" }, { "name": "beta" }],
            "transpose": [["name", "alice", "bob"], ["age", 3, null]],
            "enumerate": [
                { "index": 0, "item": "a" },
                { "index": 1, "item": "b" }
            ],
            "empty": true,
            "notEmpty": true,
            "originalValues": [1, null, "", 4],
            "originalRows": [["name", "age"], ["alice", 3], ["bob"]]
        }))
    );
}

#[test]
fn collection_reducer_helpers_ignore_non_numeric_values() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            const numbers = [1, "2", "bad", null, 4.567];
            ({
              sum: numbers.sum(),
              avg: numbers.avg(),
              min: numbers.min(),
              max: numbers.max(),
              rounded: numbers.round(1),
              compactedAvg: numbers.compact().avg(),
              emptyAvg: [].avg(),
              emptyMin: [].min(),
              emptyMax: [].max()
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "sum": 7.567,
            "avg": 2.5223333333333335,
            "compactedAvg": 2.5223333333333335,
            "min": 1,
            "max": 4.567,
            "rounded": [1, 2, null, null, 4.6],
            "emptyAvg": null,
            "emptyMin": null,
            "emptyMax": null
        }))
    );
}
