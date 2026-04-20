#![allow(clippy::expect_used)]
//! Havoc property test for SOSL builder
//!
//! This test uses proptest to hit boundaries for SOSL query builder.

#[cfg(test)]
mod tests {
    use force::api::rest::SearchQueryBuilder;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_sosl_builder_no_panic(s in "\\PC*") {
            // Should panic if input is empty, so we test only when non-empty
            if !s.trim().is_empty() {
                let _ = SearchQueryBuilder::new()
                    .find(&s)
                    .in_all_fields()
                    .returning("Account", &["Id".to_string()])
                    .build();
            }
        }
    }
}
