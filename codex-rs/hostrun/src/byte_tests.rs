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
              u16le: [0x34, 0x12].u16le(),
              u16be: [0x12, 0x34].u16be(),
              u32le: [0x78, 0x56, 0x34, 0x12].u32le(),
              u32be: [0x12, 0x34, 0x56, 0x78].u32be(),
              i32le: [0xff, 0xff, 0xff, 0xff].i32le(),
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
            "u16le": 4660,
            "u16be": 4660,
            "u32le": 305419896,
            "u32be": 305419896,
            "i32le": -1,
            "byteLengths": [1, 2, 4]
        }))
    );
}
