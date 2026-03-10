//! Havoc tests
use force::api::rest::SoqlQueryBuilder;

#[test]
fn test_havoc_soql_try_apis() {
    let b = SoqlQueryBuilder::new();
    let res = b.clone().try_where_eq("", "test");
    assert!(res.is_err());

    let res = b.clone().try_where_in("\x00", &["test"]);
    assert!(res.is_err());

    let res = b.try_order_by("");
    assert!(res.is_err());
}
