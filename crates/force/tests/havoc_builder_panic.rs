//! 👺 Havoc: `SoqlQueryBuilder` Panic Analysis

#[cfg(test)]
mod tests {
    use force::api::SoqlQueryBuilder;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10000))]
        #[test]
        fn havoc_soql_chain_methods_never_panic(field in ".*", value in ".*") {
            // These should NOT panic!
            let _ = SoqlQueryBuilder::new().from("Account").where_eq(&field, &value);
            let _ = SoqlQueryBuilder::new().from("Account").where_ne(&field, &value);
            let _ = SoqlQueryBuilder::new().from("Account").where_like(&field, &value);
        }
    }
}
