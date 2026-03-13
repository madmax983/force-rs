//! Havoc test for panic vulnerabilities in `SoqlQueryBuilder` and `SearchQueryBuilder` field validation

use force::api::rest::SoqlQueryBuilder;
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_soql_try_where_eq_no_panic(field in "[^a-zA-Z0-9_.]+") {
        // Havoc: Passing untrusted input to try_where_eq should return Err safely without panicking.
        let builder = SoqlQueryBuilder::new().from("Account");
        let result = builder.try_where_eq(&field, "value");
        prop_assert!(result.is_err());
    }
}
