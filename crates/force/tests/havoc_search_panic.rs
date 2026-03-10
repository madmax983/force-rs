//! Tests for havoc cases.
use force::api::rest::search::SearchQueryBuilder;

#[test]
#[should_panic(expected = "SObject name cannot be empty")]
fn test_havoc_search_panic_empty_sobject() {
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("", &["Id"]);
}

#[test]
#[should_panic(expected = "field name contains invalid character")]
fn test_havoc_search_panic_invalid_field() {
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["\x0E"]);
}
