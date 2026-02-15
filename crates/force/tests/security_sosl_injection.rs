#![allow(missing_docs)]

use force::api::rest::search::SearchQueryBuilder;

#[test]
#[should_panic(expected = "invalid characters in object name")]
fn test_returning_panics_on_invalid_sobject() {
    let _ = SearchQueryBuilder::new()
        .returning("Account; DROP TABLE Users", &["Id"]);
}

#[test]
#[should_panic(expected = "invalid characters in field name")]
fn test_returning_panics_on_invalid_field() {
    let _ = SearchQueryBuilder::new()
        .returning("Account", &["Id; DROP TABLE Users"]);
}
