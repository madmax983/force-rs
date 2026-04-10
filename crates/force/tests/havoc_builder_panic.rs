//! 👺 Havoc: `SoqlQueryBuilder` Panic
//!
//! **The Trigger:** Passing arbitrary strings (e.g., from an untrusted client) into
//! `where_eq`, `where_in`, `order_by`, etc.
//! **The Stack Trace:** Panic at `validate_field` unwrapping inside the library instead
//! of returning a recoverable `Result`.
//! **Reproduction:** Run `cargo test -p force --test havoc_builder_panic`
//! **Comment:** You assumed the user would always provide valid field names. You were wrong.

#![allow(clippy::unwrap_used)]

#[cfg(test)]
mod tests {
    use force::api::rest::SoqlQueryBuilder;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_soql_where_eq_panic(field in ".*", value in ".*") {
            // This MUST NOT panic, it should return an Err if the field is invalid.
            // But currently `where_eq` uses `unwrap_or_panic`, causing a crash.
            let builder = SoqlQueryBuilder::new().select(&["Id"]).from("Account");
            let _ = builder.where_eq(&field, &value);
        }

        #[test]
        fn test_soql_where_in_panic(field in ".*", values in prop::collection::vec(".*", 0..5)) {
            let builder = SoqlQueryBuilder::new().select(&["Id"]).from("Account");
            let _ = builder.where_in(&field, &values);
        }

        #[test]
        fn test_soql_order_by_panic(field in ".*") {
            let builder = SoqlQueryBuilder::new().select(&["Id"]).from("Account");
            let _ = builder.order_by(&field);
        }
    }
}
