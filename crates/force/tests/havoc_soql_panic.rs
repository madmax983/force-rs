//! Fuzz testing `SoqlQueryBuilder` panicking methods.
//!
//! # 👺 Havoc: `SoqlQueryBuilder` Panic
//!
//! **The Trigger:** Passing arbitrary strings (e.g., from an untrusted client) into
//! `where_eq`, `where_in`, `order_by`, etc.
//! **The Stack Trace:** Panic at `validate_field` unwrapping inside the library instead
//! of returning a recoverable `Result`.

#[cfg(test)]
mod tests {
    use force::api::soql::SoqlQueryBuilder;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10000))]

        #[test]
        fn test_soql_where_eq_no_panic(field in ".*", value in ".*") {
            let builder = SoqlQueryBuilder::new().from("Account");

            // This MUST NOT panic, it should return an Err if the field is invalid.
            let _ = builder.where_eq(&field, &value);
        }

        #[test]
        fn test_soql_where_in_no_panic(field in ".*", values in prop::collection::vec(".*", 0..5)) {
            let builder = SoqlQueryBuilder::new().from("Account");
            let _ = builder.where_in(&field, &values);
        }

        #[test]
        fn test_soql_order_by_no_panic(field in ".*") {
            let builder = SoqlQueryBuilder::new().from("Account");
            let _ = builder.order_by(&field);
            let builder2 = SoqlQueryBuilder::new().from("Account");
            let _ = builder2.order_by_desc(&field);
        }
    }
}
