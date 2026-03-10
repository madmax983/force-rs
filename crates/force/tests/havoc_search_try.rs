//! Havoc tests
use force::api::rest::search::SearchQueryBuilder;

#[test]
fn test_havoc_search_try_apis() {
    let b = SearchQueryBuilder::new();
    let res = b.try_returning("", &["Id"]);
    assert!(res.is_err());

    let b = SearchQueryBuilder::new();
    let res = b.try_returning("Account", &["\x0E"]);
    assert!(res.is_err());

    let b = SearchQueryBuilder::new().find("");
    let res = b.try_build();
    assert!(res.is_err());
}
