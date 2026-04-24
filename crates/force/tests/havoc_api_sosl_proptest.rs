//! 👺 Havoc: `SearchQueryBuilder` Panic Analysis

#[cfg(test)]
mod tests {
    use force::api::rest::SearchQueryBuilder;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10000))]
        #[test]
        fn havoc_sosl_never_panics_on_any_string(s in ".*") {
            if !s.trim().is_empty() {
                let _ = SearchQueryBuilder::new().find(&s).in_all_fields().try_returning("Account", &["Id"]);
            }
        }
    }
}
