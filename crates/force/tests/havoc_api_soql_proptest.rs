//! Havoc property test for SOQL escaping
//!
//! This test uses proptest to hit boundaries for SOQL escape functions.

#[cfg(test)]
mod tests {
    use force::api::escape_soql;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_escape_soql_no_panic(s in "\\PC*") {
            let escaped = escape_soql(&s);
            assert!(escaped.len() >= s.len());
        }
    }
}
