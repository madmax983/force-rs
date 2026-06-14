//! Havoc test for simulating DOS vectors using massive field arrays.

use force::http::parse_api_error;
use std::fmt::Write;

#[test]
fn havoc_parse_api_error_overflow() {
    let large_field = "A".repeat(10_000);
    let mut fields_json = String::new();
    fields_json.push('[');
    for i in 0..10_000 {
        if i > 0 {
            fields_json.push(',');
        }
        let _ = write!(fields_json, r#""{large_field}""#);
    }
    fields_json.push(']');

    let body = format!(r#"[{{ "errorCode": "ERR", "message": "Msg", "fields": {fields_json} }}]"#);

    let _ = parse_api_error(400, &body);
}
