//! Havoc regression test for SOSL injection panic.

use force::api::rest::search::SearchQueryBuilder;

#[test]
fn test_havoc_sosl_try_returning() {
    let result = SearchQueryBuilder::new()
        .find("test")
        .try_returning("Account", &["unbalanced_closing_parentheses)"]);

    assert!(result.is_err(), "👺 Havoc: User input should return an error, not panic!");
}
