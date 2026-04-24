//! 👺 Havoc: `SoqlQueryBuilder` Panic Analysis

#[cfg(test)]
mod tests {
    use force::api::SoqlQueryBuilder;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10000))]
        #[test]
        fn havoc_soql_never_panics_on_any_string(s in ".*") {
            let _ = SoqlQueryBuilder::new().try_where_eq("Id", &s);
            let _ = SoqlQueryBuilder::new().try_where_ne("Id", &s);
            let _ = SoqlQueryBuilder::new().try_where_like("Id", &s);
            let _ = SoqlQueryBuilder::new().try_where_in("Id", &[s.as_str()]);
            let _ = force::api::escape_soql(&s);
        }
    }
}
