#![allow(missing_docs)]

use force::api::rest::search::SearchQueryBuilder;

#[test]
#[should_panic(expected = "Object name contains invalid characters")]
fn test_search_query_builder_injection_object() {
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account; DROP TABLE", &["Id"]);
}

#[test]
#[should_panic(expected = "Field name contains invalid characters")]
fn test_search_query_builder_injection_field() {
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["toLabel(Name)"]);
}

#[test]
#[should_panic(expected = "Field name contains invalid characters")]
fn test_search_query_builder_injection_complex_field() {
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["count()"]);
}
