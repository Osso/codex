use serde_json::json;

use super::HostrunSession;

#[test]
fn byte_helpers_expose_utf8_bytes_and_ranges() {
    let session = HostrunSession::new().expect("session");

    let result = session
        .eval(
            r#"
            ({
              asciiBytes: "ABC".byteArray(),
              unicodeBytes: "é𝄞".byteArray(),
              stringRange: "AéB".byteRange(1, 2),
              singleByte: "AéB".byteRange(3),
              arrayRange: [0x10, 0x20, 0x30, 0x40].byteRange(1, 2),
              byteLengths: ["A", "é", "𝄞"].bytes()
            });
            "#,
        )
        .expect("eval");

    assert_eq!(
        result.value,
        Some(json!({
            "asciiBytes": [65, 66, 67],
            "unicodeBytes": [195, 169, 240, 157, 132, 158],
            "stringRange": [195, 169],
            "singleByte": [66],
            "arrayRange": [32, 48],
            "byteLengths": [1, 2, 4]
        }))
    );
}
