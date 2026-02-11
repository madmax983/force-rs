//! Salesforce ID newtype with validation.
//!
//! Salesforce IDs come in two formats:
//! - 15-character case-sensitive format
//! - 18-character case-insensitive format (15-char + 3-char checksum)
//!
//! This module provides a validated newtype wrapper ensuring ID correctness.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A validated Salesforce record ID.
///
/// Salesforce IDs are either 15 or 18 characters long and contain only
/// alphanumeric characters. The 18-character format includes a 3-character
/// case-insensitive checksum of the 15-character ID.
///
/// # Examples
///
/// ```
/// use force::types::SalesforceId;
///
/// // 15-character ID
/// let id = SalesforceId::new("001000000000001").unwrap();
/// assert_eq!(id.as_str(), "001000000000001");
///
/// // 18-character ID
/// let id = SalesforceId::new("001000000000001AAA").unwrap();
/// assert_eq!(id.as_str(), "001000000000001AAA");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SalesforceId(String);

impl SalesforceId {
    /// Creates a new validated Salesforce ID.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The ID is not 15 or 18 characters long
    /// - The ID contains non-alphanumeric characters
    /// - The 18-character checksum is invalid (when applicable)
    pub fn new(id: impl Into<String>) -> Result<Self, SalesforceIdError> {
        let id = id.into();

        // Validate length
        match id.len() {
            15 | 18 => {}
            _ => return Err(SalesforceIdError::InvalidLength(id.len())),
        }

        // Validate characters are alphanumeric
        if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(SalesforceIdError::InvalidCharacters);
        }

        // Validate 18-char checksum if applicable
        if id.len() == 18 {
            Self::validate_checksum(&id)?;
        }

        Ok(Self(id))
    }

    /// Returns the ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts a 15-character ID to its 18-character equivalent.
    ///
    /// If the ID is already 18 characters, returns a clone.
    #[must_use]
    pub fn to_18(&self) -> Self {
        if self.0.len() == 18 {
            return self.clone();
        }

        let checksum = Self::compute_checksum(&self.0);
        Self(format!("{}{}", self.0, checksum))
    }

    /// Returns the 15-character base of the ID.
    ///
    /// If the ID is already 15 characters, returns a clone.
    /// If 18 characters, returns just the first 15.
    #[must_use]
    pub fn to_15(&self) -> Self {
        if self.0.len() == 15 {
            return self.clone();
        }

        Self(self.0[..15].to_string())
    }

    /// Validates the checksum of an 18-character ID.
    fn validate_checksum(id: &str) -> Result<(), SalesforceIdError> {
        debug_assert_eq!(id.len(), 18);

        let base = &id[..15];
        let provided_checksum = &id[15..];
        let computed_checksum = Self::compute_checksum(base);

        if provided_checksum == computed_checksum {
            Ok(())
        } else {
            Err(SalesforceIdError::InvalidChecksum)
        }
    }

    /// Computes the 3-character checksum for a 15-character ID.
    ///
    /// The checksum algorithm:
    /// - Divide the 15 chars into 3 groups of 5
    /// - For each group, treat uppercase letters as 1, lowercase/digits as 0
    /// - Convert the 5-bit value to a base-32 character
    fn compute_checksum(id: &str) -> String {
        debug_assert_eq!(id.len(), 15);

        let mut checksum = String::with_capacity(3);

        for chunk in id.as_bytes().chunks(5) {
            let mut value = 0u8;
            for (i, &byte) in chunk.iter().enumerate() {
                if byte.is_ascii_uppercase() {
                    value |= 1 << i;
                }
            }
            checksum.push(Self::base32_char(value));
        }

        checksum
    }

    /// Converts a 5-bit value to a base-32 character.
    fn base32_char(value: u8) -> char {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ012345";
        CHARS[value as usize] as char
    }
}

impl fmt::Display for SalesforceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for SalesforceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<SalesforceId> for String {
    fn from(id: SalesforceId) -> Self {
        id.0
    }
}

impl TryFrom<String> for SalesforceId {
    type Error = SalesforceIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Errors that can occur when creating or validating a Salesforce ID.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SalesforceIdError {
    /// The ID length is invalid (must be 15 or 18 characters).
    #[error("invalid ID length: {0} (must be 15 or 18 characters)")]
    InvalidLength(usize),

    /// The ID contains non-alphanumeric characters.
    #[error("ID contains invalid characters (must be alphanumeric)")]
    InvalidCharacters,

    /// The 18-character checksum is invalid.
    #[error("invalid checksum for 18-character ID")]
    InvalidChecksum,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    // RED PHASE - Write failing tests first

    #[test]
    fn test_new_15_char_valid() {
        let id = SalesforceId::new("001000000000001");
        assert!(id.is_ok());
        assert_eq!(id.must().as_str(), "001000000000001");
    }

    #[test]
    fn test_new_18_char_valid() {
        // Using a real Salesforce ID with valid checksum
        let id = SalesforceId::new("001000000000001AAA");
        assert!(id.is_ok());
        assert_eq!(id.must().as_str(), "001000000000001AAA");
    }

    #[test]
    fn test_new_invalid_length_too_short() {
        let id = SalesforceId::new("00100000000");
        assert!(matches!(id, Err(SalesforceIdError::InvalidLength(11))));
    }

    #[test]
    fn test_new_invalid_length_too_long() {
        let id = SalesforceId::new("0010000000000001AAA");
        assert!(matches!(id, Err(SalesforceIdError::InvalidLength(19))));
    }

    #[test]
    fn test_new_invalid_characters() {
        let id = SalesforceId::new("001000000000@01");
        assert!(matches!(id, Err(SalesforceIdError::InvalidCharacters)));
    }

    #[test]
    fn test_new_invalid_checksum() {
        // Valid format but wrong checksum
        let id = SalesforceId::new("001000000000001XXX");
        assert!(matches!(id, Err(SalesforceIdError::InvalidChecksum)));
    }

    #[test]
    fn test_to_18_from_15() {
        let id = SalesforceId::new("001000000000001").must();
        let id_18 = id.to_18();
        assert_eq!(id_18.as_str().len(), 18);
        assert!(id_18.as_str().starts_with("001000000000001"));
    }

    #[test]
    fn test_to_18_from_18() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let id_18 = id.to_18();
        assert_eq!(id_18.as_str(), "001000000000001AAA");
    }

    #[test]
    fn test_to_15_from_15() {
        let id = SalesforceId::new("001000000000001").must();
        let id_15 = id.to_15();
        assert_eq!(id_15.as_str(), "001000000000001");
    }

    #[test]
    fn test_to_15_from_18() {
        let id = SalesforceId::new("001000000000001AAA").must();
        let id_15 = id.to_15();
        assert_eq!(id_15.as_str(), "001000000000001");
    }

    #[test]
    fn test_equality_15_and_18() {
        let id_15 = SalesforceId::new("001000000000001").must();
        let id_18 = id_15.to_18();
        // They should not be equal as they're different representations
        assert_ne!(id_15, id_18);
        // But converting to same format should be equal
        assert_eq!(id_15.to_18(), id_18);
        assert_eq!(id_15, id_18.to_15());
    }

    #[test]
    fn test_display_trait() {
        let id = SalesforceId::new("001000000000001").must();
        assert_eq!(format!("{}", id), "001000000000001");
    }

    #[test]
    fn test_as_ref_trait() {
        let id = SalesforceId::new("001000000000001").must();
        let s: &str = id.as_ref();
        assert_eq!(s, "001000000000001");
    }

    #[test]
    fn test_checksum_computation() {
        // Test with known Salesforce IDs
        // These are real checksum examples from Salesforce documentation
        let test_cases = vec![
            ("001D000000IRt53", "001D000000IRt53IAD"),
            ("003D000000QqeJZ", "003D000000QqeJZIAZ"),
        ];

        for (base, expected_full) in test_cases {
            let id = SalesforceId::new(base).must();
            let id_18 = id.to_18();
            assert_eq!(id_18.as_str(), expected_full);

            // Also verify we can parse the 18-char version
            let parsed = SalesforceId::new(expected_full).must();
            assert_eq!(parsed.as_str(), expected_full);
        }
    }

    #[test]
    fn test_case_sensitivity() {
        // 15-char IDs are case-sensitive
        let id1 = SalesforceId::new("001D000000IRt53").must();
        let id2 = SalesforceId::new("001d000000irt53").must();
        assert_ne!(id1, id2);

        // Their checksums should differ
        assert_ne!(id1.to_18().as_str(), id2.to_18().as_str());
    }

    // Property-based tests using proptest
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        // Strategy to generate valid 15-character Salesforce IDs
        fn valid_15_char_id() -> impl Strategy<Value = String> {
            prop::collection::vec(
                prop::char::range('0', 'z')
                    .prop_filter("Must be alphanumeric", |c| c.is_ascii_alphanumeric()),
                15..=15,
            )
            .prop_map(|chars| chars.into_iter().collect())
        }

        // Strategy to generate invalid length strings
        fn invalid_length_string() -> impl Strategy<Value = String> {
            prop::collection::vec(prop::char::range('a', 'z'), 1..100)
                .prop_filter("Must not be 15 or 18 chars", |v| {
                    v.len() != 15 && v.len() != 18
                })
                .prop_map(|chars| chars.into_iter().collect())
        }

        proptest! {
            // Property 1: 15-char -> to_18() -> to_15() roundtrip
            #[test]
            fn prop_roundtrip_15_to_18_to_15(id_str in valid_15_char_id()) {
                let id_15 = SalesforceId::new(&id_str).must();
                let id_18 = id_15.to_18();
                let back_to_15 = id_18.to_15();

                prop_assert_eq!(id_15.as_str(), back_to_15.as_str());
            }

            // Property 2: 18-char -> to_15() -> to_18() roundtrip
            #[test]
            fn prop_roundtrip_18_to_15_to_18(id_str in valid_15_char_id()) {
                let id_15 = SalesforceId::new(&id_str).must();
                let id_18 = id_15.to_18();
                let id_18_str = id_18.as_str().to_string();

                // Now roundtrip from 18
                let back_to_15 = id_18.to_15();
                let back_to_18 = back_to_15.to_18();

                prop_assert_eq!(id_18_str, back_to_18.as_str());
            }

            // Property 3: to_18() always produces valid 18-char ID
            #[test]
            fn prop_to_18_produces_valid_id(id_str in valid_15_char_id()) {
                let id_15 = SalesforceId::new(&id_str).must();
                let id_18 = id_15.to_18();

                prop_assert_eq!(id_18.as_str().len(), 18);

                // Should be parseable as valid ID
                let reparsed = SalesforceId::new(id_18.as_str());
                prop_assert!(reparsed.is_ok());
            }

            // Property 4: Invalid lengths always reject
            #[test]
            fn prop_invalid_length_rejects(id_str in invalid_length_string()) {
                let result = SalesforceId::new(&id_str);

                prop_assert!(result.is_err());
                if let Err(SalesforceIdError::InvalidLength(len)) = result {
                    prop_assert_eq!(len, id_str.len());
                }
            }

            // Property 5: IDs with non-alphanumeric chars reject
            #[test]
            fn prop_non_alphanumeric_rejects(
                prefix in "[a-zA-Z0-9]{7}",
                special_char in "[@#$%^&*()!]",  // ASCII special chars only
                suffix in "[a-zA-Z0-9]{7}"
            ) {
                let id_str = format!("{}{}{}", prefix, special_char, suffix);
                let result = SalesforceId::new(&id_str);

                prop_assert!(matches!(result, Err(SalesforceIdError::InvalidCharacters)));
            }

            // Property 6: to_15() is idempotent for 15-char IDs
            #[test]
            fn prop_to_15_idempotent_on_15_char(id_str in valid_15_char_id()) {
                let id = SalesforceId::new(&id_str).must();
                let once = id.to_15();
                let twice = once.to_15();

                prop_assert_eq!(once.as_str(), twice.as_str());
                prop_assert_eq!(once.as_str(), id_str);
            }

            // Property 7: to_18() is idempotent for 18-char IDs
            #[test]
            fn prop_to_18_idempotent_on_18_char(id_str in valid_15_char_id()) {
                let id_15 = SalesforceId::new(&id_str).must();
                let id_18 = id_15.to_18();
                let id_18_str = id_18.as_str().to_string();

                let once = id_18.to_18();
                let once_str = once.as_str().to_string();
                let twice = once.to_18();

                prop_assert_eq!(once_str, twice.as_str());
                prop_assert_eq!(id_18_str, twice.as_str());
            }

            // Property 8: Display and as_str are consistent
            #[test]
            fn prop_display_consistent_with_as_str(id_str in valid_15_char_id()) {
                let id = SalesforceId::new(&id_str).must();
                let displayed = format!("{}", id);

                prop_assert_eq!(displayed, id.as_str());
            }

            // Property 9: Checksum validation catches corruption
            #[test]
            fn prop_bad_checksum_rejects(
                id_str in valid_15_char_id(),
                bad_checksum in "[A-Z0-5]{3}"
            ) {
                let id_15 = SalesforceId::new(&id_str).must();
                let id_18 = id_15.to_18();
                let id_18_str = id_18.as_str().to_string();
                let correct_checksum = &id_18_str[15..];

                // Only test if we actually generated a different checksum
                prop_assume!(bad_checksum != correct_checksum);

                let bad_id = format!("{}{}", &id_18_str[..15], bad_checksum);
                let result = SalesforceId::new(&bad_id);

                prop_assert!(matches!(result, Err(SalesforceIdError::InvalidChecksum)));
            }
        }
    }
}
