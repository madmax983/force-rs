//! Fuzz testing `SoqlQueryBuilder` panicking methods.
//!
//! # 👺 Havoc: `SoqlQueryBuilder` Panic
//!
//! **The Trigger:** Fuzzing the `try_where_eq`, `try_where_in`, and `try_select` API,
//! as well as the low level `escape_soql_cow` function.
//! **The Stack Trace:** Checking for memory explosions or panics deep in the validation.

#[cfg(test)]
mod tests {
    use force::api::soql::{SoqlQueryBuilder, escape_soql_cow};
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10000))]

        #[test]
        fn test_soql_fuzz_where_eq(field in ".*", value in ".*") {
            let _ = SoqlQueryBuilder::new().try_where_eq(&field, &value);
        }

        #[test]
        fn test_soql_fuzz_where_in(field in ".*", values in prop::collection::vec(".*", 0..20)) {
            let _ = SoqlQueryBuilder::new().try_where_in(&field, &values);
        }

        #[test]
        fn test_soql_fuzz_try_select(fields in prop::collection::vec(".*", 0..20)) {
            let _ = SoqlQueryBuilder::new().try_select(&fields);
        }

        #[test]
        fn test_escape_soql_fuzz(input in ".*") {
            let _ = escape_soql_cow(&input);
        }
    }
}
