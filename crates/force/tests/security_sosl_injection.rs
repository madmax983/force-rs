#![allow(missing_docs)]
#![cfg(feature = "rest")]

use force::api::rest::search::SearchQueryBuilder;

#[test]
#[should_panic(expected = "Invalid object name")]
fn test_search_query_builder_injection_repro() {
    // This test attempts to inject a malicious clause into the RETURNING statement.
    // If the builder does not validate the input, this will produce a valid SOSL query
    // that returns data from an unintended object (Contact), which is a vulnerability.
    //
    // The test expects a panic because a secure implementation should reject this input.
    // Currently, it does not panic, so this test will fail (demonstrating the vulnerability).

    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account(Id), Contact(Name)", &[] as &[&str])
        .build();
}

#[test]
#[should_panic(expected = "Invalid field name")]
fn test_search_query_builder_field_injection_repro() {
    // Similar injection attempt via fields
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["Id), Contact(Name"])
        .build();
}
