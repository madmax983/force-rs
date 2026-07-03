//! Havoc property test for SOQL builder limits
//!
//! This test uses proptest to hit boundaries for SOQL limit/offset serialization.

#[cfg(test)]
mod tests {
    use force::api::SoqlQueryBuilder;

    #[test]
    fn test_soql_where_in_dos_panic() {
        let values: Vec<String> = vec!["A".to_string(); 500_000];
        let result = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from("Account")
            .try_where_in("Id", &values);

        assert!(matches!(
            result,
            Err(force::error::ForceError::InvalidInput(_))
        ));
    }
}
