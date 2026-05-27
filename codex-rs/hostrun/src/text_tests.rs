use serde_json::json;

use super::HostrunSession;

#[test]
fn text_helpers_cover_common_head_tail_split_and_replace_workflows() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            const text = "  alpha  beta\ncarol  delta\nedgar  frank  ";
            const lines = text.lines();
            ({
              lineCount: text.lineCount(),
              wordCount: text.wordCount(),
              byteCount: "é\n".byteCount(),
              head: text.head(2),
              tail: text.tail(1),
              cut: "name,kind,count\nalpha,comic,12".cut(",", [1, 3]),
              splitRow: "a,b,c".splitRow(","),
              splitWords: "  one   two\tthree ".splitWords(),
              splitColumn: text.splitColumn(/\s+/, ["first", "second"]),
              trimmed: text.trimmed(),
              replaced: "a-b-a".replaceText("a", "z"),
              arrayHead: lines.head(1),
              arrayTail: lines.tail(2),
              joined: ["a", "b", "c"].joinText("|")
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "lineCount": 3,
            "wordCount": 6,
            "byteCount": 3,
            "head": ["  alpha  beta", "carol  delta"],
            "tail": ["edgar  frank  "],
            "cut": [["name", "count"], ["alpha", "12"]],
            "splitRow": ["a", "b", "c"],
            "splitWords": ["one", "two", "three"],
            "splitColumn": [
                { "first": "alpha", "second": "beta" },
                { "first": "carol", "second": "delta" },
                { "first": "edgar", "second": "frank" }
            ],
            "trimmed": "alpha  beta\ncarol  delta\nedgar  frank",
            "replaced": "z-b-z",
            "arrayHead": ["  alpha  beta"],
            "arrayTail": ["carol  delta", "edgar  frank  "],
            "joined": "a|b|c"
        }))
    );
}
