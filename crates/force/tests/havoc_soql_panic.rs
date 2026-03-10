//! Tests for havoc cases.
use force::api::rest::SoqlQueryBuilder;

#[test]
#[should_panic(expected = "Invalid input in where_eq")]
fn test_havoc_soql_panic_where_eq() {
    let _ = SoqlQueryBuilder::new().where_eq("", "test");
}

#[test]
#[should_panic(expected = "Invalid input in where_in")]
fn test_havoc_soql_panic_where_in() {
    let _ = SoqlQueryBuilder::new().where_in("\x00", &["test"]);
}
