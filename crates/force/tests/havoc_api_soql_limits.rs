//! Havoc property test for SOQL builder limits
//!
//! This test uses proptest to hit boundaries for SOQL limit/offset serialization.

#[cfg(test)]
mod tests {
    use force::api::SoqlQueryBuilder;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_soql_limit_offset_no_panic(limit in 0..u32::MAX, offset in 0..u32::MAX) {
            let _ = SoqlQueryBuilder::new()
                .select(&["Id"])
                .from("Account")
                .limit(limit)
                .offset(offset)
                .build();
        }
    }
}
