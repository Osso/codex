use serde_json::json;

use super::HostrunSession;

#[test]
fn object_table_projection_helpers_do_not_mutate_inputs() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            const rows = [
              { name: "alpha", status: "active", metadata: { name: "pod-a" }, tags: ["api", "blue"], age: 3 },
              { name: "beta", status: "inactive", metadata: { name: "pod-b" }, tags: ["worker"], age: 5 },
              { name: "gamma", status: "active", metadata: { name: "pod-c" }, tags: ["api"], age: 8 }
            ];
            const originalRecord = { name: "alpha", status: "active", age: 3 };
            const updated = originalRecord
              .rename({ name: "pod" })
              .insert("namespace", "default")
              .update("age", (age) => age + 1)
              .merge({ ready: true });
            ({
              nestedNames: rows.get("metadata.name"),
              pluckedNames: rows.pluck("metadata.name"),
              secondTag: rows.valuesOf("tags.1"),
              selected: rows.select("name", "metadata.name"),
              rejected: rows.reject("tags", "age"),
              matching: rows.where({ status: "active", "metadata.name": "pod-c" }).get("name"),
              predicate: rows.where((row) => row.age > 3).get("name"),
              columns: rows.columns(),
              recordGet: rows[0].get("metadata.name"),
              recordSelected: originalRecord.select("name", "status"),
              recordRejected: originalRecord.reject("age"),
              updated,
              recordColumns: updated.columns(),
              recordValues: updated.values(),
              recordEntries: updated.entries(),
              recordItems: updated.items(),
              rowsAfter: rows,
              originalRecord
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "nestedNames": ["pod-a", "pod-b", "pod-c"],
            "pluckedNames": ["pod-a", "pod-b", "pod-c"],
            "secondTag": ["blue", null, null],
            "selected": [
                { "name": "alpha", "metadata.name": "pod-a" },
                { "name": "beta", "metadata.name": "pod-b" },
                { "name": "gamma", "metadata.name": "pod-c" }
            ],
            "rejected": [
                { "name": "alpha", "status": "active", "metadata": { "name": "pod-a" } },
                { "name": "beta", "status": "inactive", "metadata": { "name": "pod-b" } },
                { "name": "gamma", "status": "active", "metadata": { "name": "pod-c" } }
            ],
            "matching": ["gamma"],
            "predicate": ["beta", "gamma"],
            "columns": ["name", "status", "metadata", "tags", "age"],
            "recordGet": "pod-a",
            "recordSelected": { "name": "alpha", "status": "active" },
            "recordRejected": { "name": "alpha", "status": "active" },
            "updated": { "pod": "alpha", "status": "active", "age": 4, "namespace": "default", "ready": true },
            "recordColumns": ["pod", "status", "age", "namespace", "ready"],
            "recordValues": ["alpha", "active", 4, "default", true],
            "recordEntries": [["pod", "alpha"], ["status", "active"], ["age", 4], ["namespace", "default"], ["ready", true]],
            "recordItems": [["pod", "alpha"], ["status", "active"], ["age", 4], ["namespace", "default"], ["ready", true]],
            "rowsAfter": [
                { "name": "alpha", "status": "active", "metadata": { "name": "pod-a" }, "tags": ["api", "blue"], "age": 3 },
                { "name": "beta", "status": "inactive", "metadata": { "name": "pod-b" }, "tags": ["worker"], "age": 5 },
                { "name": "gamma", "status": "active", "metadata": { "name": "pod-c" }, "tags": ["api"], "age": 8 }
            ],
            "originalRecord": { "name": "alpha", "status": "active", "age": 3 }
        }))
    );
}
