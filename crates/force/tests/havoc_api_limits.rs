//! 👺 Havoc: `SearchQueryBuilder` and `SoqlQueryBuilder` limit panics
//!

#[cfg(test)]
mod tests {
    use force::api::SoqlQueryBuilder;
    use force::api::rest::SearchQueryBuilder;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10000))]

        #[test]
        fn test_soql_limit_offset_no_panic(limit in 0..u32::MAX, offset in 0..u32::MAX) {
            let _ = SoqlQueryBuilder::new()
                .select(&["Id"])
                .from("Account")
                .limit(limit)
                .offset(offset)
                .build();
        }

        #[test]
        fn test_sosl_limit_offset_no_panic(limit in 0..u32::MAX, offset in 0..u32::MAX) {
            let _ = SearchQueryBuilder::new()
                .find("Acme")
                .returning("Account", &["Id"])
                .limit(limit)
                .offset(offset)
                .build();
        }
    }
}
