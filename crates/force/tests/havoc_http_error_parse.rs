//! Havoc test for parsing unexpected JSON structures in HTTP errors.

use force::http::parse_api_error;

#[test]
fn havoc_parse_api_error_nested_array() {
    let body = r#"[["errorCode", "INVALID_FIELD"]]"#;
    let _ = parse_api_error(400, body);
}

#[test]
fn havoc_parse_api_error_invalid_type() {
    let body = r#"[{"errorCode": 123, "message": 456, "fields": [789]}]"#;
    let _ = parse_api_error(400, body);
}
