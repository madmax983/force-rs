//! Fuzz testing `SearchQueryBuilder` parsing of wild inputs.
//!
//! # 👺 Havoc: `SearchQueryBuilder` Memory/Panic
//!
//! **The Trigger:** Fuzzing the `try_returning` method with arbitrary length inputs, and the underlying escape helper.
//! **The Stack Trace:** Checking for memory explosions or panics deep in the validation.

#[cfg(test)]
mod tests {
    use force::api::rest::SearchQueryBuilder;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10000))]

        #[test]
        fn test_search_try_returning_fuzz(sobject in ".*", fields in prop::collection::vec(".*", 0..20)) {
            let builder = SearchQueryBuilder::new().find("Test");
            let _ = builder.try_returning(&sobject, &fields);
        }

        #[test]
        fn test_search_escape_sosl_fuzz(text in ".*") {
            let builder = SearchQueryBuilder::new().find(&text);
            let _ = builder.try_build();
        }
    }
}
