#![allow(missing_docs)]
#![cfg(feature = "rest")]
//! Tests for SOQL injection prevention.

use force::api::rest::SoqlQueryBuilder;

#[test]
fn test_soql_injection_prevention() {
    // Scenario: User input contains a single quote, which could break out of the string literal
    let malicious_input = "O'Reilly";

    // Expected safe query: SELECT Id, Name FROM Account WHERE Name = 'O\'Reilly'
    // (Note: SOQL uses backslash to escape single quotes)

    let query = SoqlQueryBuilder::new()
        .select(&["Id", "Name"])
        .from("Account")
        .where_eq("Name", malicious_input)
        .build();

    assert_eq!(
        query,
        "SELECT Id, Name FROM Account WHERE Name = 'O\\'Reilly'"
    );
}

#[test]
fn test_soql_injection_complex_chars() {
    // Scenario: User input contains backslashes and quotes
    let complex_input = r"C:\Windows\'System32'";

    // We expect backslashes to be escaped as well
    let query = SoqlQueryBuilder::new()
        .select(&["Id"])
        .from("Document")
        .where_eq("Path", complex_input)
        .build();

    // Expected: ... WHERE Path = 'C:\\Windows\\\'System32\''
    // In Rust string literal, backslashes are escaped, so:
    // 'C:\\\\Windows\\\\\\'System32\''

    // Let's verify the string content directly
    assert!(query.contains(r"C:\\Windows\\\'System32\'"));
}
