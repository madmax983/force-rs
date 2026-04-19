//! Havoc property test for SOQL escaping
//!
//! This test uses proptest to hit boundaries for SOQL escape functions.

#[cfg(test)]
mod tests {
    use force::api::soql::{escape_soql, escape_soql_cow};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_escape_soql_no_panic(s in "\\PC*") {
            let escaped = escape_soql(&s);
            let cow = escape_soql_cow(&s);
            assert_eq!(escaped, cow.into_owned());
        }
    }
}
