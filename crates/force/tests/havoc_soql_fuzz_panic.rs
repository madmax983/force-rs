//! 👺 Havoc: SOQL query builder panics

#[test]
fn test_soql_panic() {
    let _builder = force::api::SoqlQueryBuilder::new().select(&["Name; DROP TABLE Account"]);
}
