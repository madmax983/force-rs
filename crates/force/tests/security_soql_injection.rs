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

#[test]
fn test_soql_injection_real_world_vectors() {
    // List of common SQL injection payloads adapted for SOQL
    let attack_vectors = vec![
        "' OR '1'='1",
        "admin' --",
        "test' AND (SELECT count(Id) FROM User) > 0 --",
        r"\' ); DROP TABLE Account; --",
        "test' OR Name LIKE '%",
    ];

    for vector in attack_vectors {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("User")
            .where_eq("Username", vector)
            .build();

        // 1. Assert the raw vector is NOT present (meaning it was escaped)
        // We check for the vector surrounded by the context of the query structure to avoid false positives
        // e.g., we expect `Username = '...vector...'`
        // If injection succeeded, we might see `Username = '' OR '1'='1'`

        // More simply: The value in the query MUST be the escaped version.
        // We can verify that the raw vector string does not appear unescaped.

        // For `' OR '1'='1`, the escaped version is `\' OR \'1\'=\'1`.
        // If the raw version appears, we have a problem.
        // Note: We need to be careful because the vector *content* is present, just escaped.
        // So `query.contains(vector)` might be true depending on the vector characters.
        // e.g. "admin' --" becomes "admin\' --". "admin' --" is NOT a substring of "admin\' --".

        // However, strictly speaking, we want to ensure the *semantics* are preserved as a string literal.
        // The best check is that the query ends with the properly closed string literal of the escaped value.

        // Let's manually escape to compare
        let escaped = vector.replace('\\', "\\\\").replace('\'', "\\'");
        let expected_clause = format!("Username = '{escaped}'");

        assert!(
            query.contains(&expected_clause),
            "Failed to escape vector: {vector}\nQuery: {query}\nExpected clause: {expected_clause}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn test_where_condition_unchecked_exploit() {
    let malicious_input = "' OR '1'='1";
    let query = SoqlQueryBuilder::new()
        .select(&["Id"])
        .from("User")
        .where_condition_unchecked(format!("Username = '{malicious_input}'"))
        .build();
    assert_eq!(query, "SELECT Id FROM User WHERE Username = '' OR '1'='1'");
}
