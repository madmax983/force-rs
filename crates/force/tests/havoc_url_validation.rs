use force::types::validator::validate_url_path;

#[test]
fn test_ssrf_payloads() {
    let payloads = vec![
        "javascript:alert(1)",
        "https:evil.com",
        "file:///etc/passwd",
        "gopher://localhost:11211/1stats",
        "data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==",
    ];

    for payload in payloads {
        let result = validate_url_path(payload);
        assert!(result.is_err(), "Payload {} should be rejected as invalid", payload);
    }
}
