//! Salesforce API version newtype with validation.
//!
//! Salesforce API versions follow the format "vX.0" where X is the major version number.
//! For example: v60.0, v61.0, v62.0, etc.
//!
//! This module provides a validated newtype wrapper ensuring version format correctness.

use std::fmt;
use std::str::FromStr;

/// Compatibility tier for a Salesforce API version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiVersionSupportTier {
    /// Version is within the library's guaranteed tested window.
    Tested,
    /// Version is supported but outside the tested compatibility window.
    SupportedUntested,
    /// Version is below the minimum supported window.
    Unsupported,
}

/// A validated Salesforce API version.
///
/// API versions follow the format "vX.0" where X is a positive integer.
/// The minimum supported version is v1.0, though in practice most clients
/// use much newer versions (v50.0+).
///
/// # Examples
///
/// ```
/// use force::types::ApiVersion;
///
/// // Parse from string
/// let version: ApiVersion = "v60.0".parse().unwrap();
/// assert_eq!(version.as_str(), "v60.0");
/// assert_eq!(version.major(), 60);
///
/// // Create from major version number
/// let version = ApiVersion::new(61);
/// assert_eq!(version.as_str(), "v61.0");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ApiVersion {
    major: u16,
}

impl ApiVersion {
    /// Creates a new API version from a major version number.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::types::ApiVersion;
    ///
    /// let version = ApiVersion::new(60);
    /// assert_eq!(version.as_str(), "v60.0");
    /// ```
    #[must_use]
    pub const fn new(major: u16) -> Self {
        Self { major }
    }

    /// Returns the major version number.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::types::ApiVersion;
    ///
    /// let version = ApiVersion::new(60);
    /// assert_eq!(version.major(), 60);
    /// ```
    #[must_use]
    pub const fn major(&self) -> u16 {
        self.major
    }

    /// Returns the version as a string in "vX.0" format.
    ///
    /// Note: This allocates a new String. For formatting purposes,
    /// prefer using the `Display` trait which is more efficient.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::types::ApiVersion;
    ///
    /// let version = ApiVersion::new(60);
    /// assert_eq!(version.as_str(), "v60.0");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> String {
        format!("v{}.0", self.major)
    }

    /// The latest stable API version (v60.0 as of implementation).
    ///
    /// This should be updated periodically as new Salesforce releases occur.
    /// Salesforce typically releases 3 versions per year (Winter, Spring, Summer).
    #[must_use]
    pub const fn latest() -> Self {
        Self::new(60)
    }

    /// Minimum API version supported by this crate.
    ///
    /// Requests below this version are outside compatibility guarantees.
    pub const MIN_SUPPORTED: Self = Self::V55;

    /// Highest API version covered by the crate's compatibility test matrix.
    pub const MAX_TESTED: Self = Self::V60;

    /// Default API version used by client configuration.
    pub const DEFAULT: Self = Self::V60;

    /// API version 60.0 (Winter '24).
    pub const V60: Self = Self::new(60);
    /// API version 59.0 (Summer '23).
    pub const V59: Self = Self::new(59);
    /// API version 58.0 (Spring '23).
    pub const V58: Self = Self::new(58);
    /// API version 57.0 (Winter '23).
    pub const V57: Self = Self::new(57);
    /// API version 56.0 (Summer '22).
    pub const V56: Self = Self::new(56);
    /// API version 55.0 (Spring '22).
    pub const V55: Self = Self::new(55);

    /// Returns true if this version is supported by the crate.
    #[must_use]
    pub const fn is_supported(self) -> bool {
        self.major >= Self::MIN_SUPPORTED.major
    }

    /// Returns true if this version is in the tested compatibility matrix window.
    #[must_use]
    pub const fn is_tested(self) -> bool {
        self.major >= Self::MIN_SUPPORTED.major && self.major <= Self::MAX_TESTED.major
    }

    /// Returns compatibility tier for this version.
    #[must_use]
    pub const fn support_tier(self) -> ApiVersionSupportTier {
        if !self.is_supported() {
            ApiVersionSupportTier::Unsupported
        } else if self.is_tested() {
            ApiVersionSupportTier::Tested
        } else {
            ApiVersionSupportTier::SupportedUntested
        }
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.0", self.major)
    }
}

impl FromStr for ApiVersion {
    type Err = ApiVersionError;

    /// Parses an API version string in "vX.0" format.
    ///
    /// # Examples
    ///
    /// ```
    /// use force::types::ApiVersion;
    ///
    /// let version: ApiVersion = "v60.0".parse().unwrap();
    /// assert_eq!(version.major(), 60);
    ///
    /// assert!("v60".parse::<ApiVersion>().is_err());
    /// assert!("60.0".parse::<ApiVersion>().is_err());
    /// assert!("v60.1".parse::<ApiVersion>().is_err());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Must start with 'v'
        let s = s.strip_prefix('v').ok_or(ApiVersionError::MissingPrefix)?;

        // Must end with '.0'
        let s = s.strip_suffix(".0").ok_or(ApiVersionError::InvalidFormat)?;

        // Parse the major version number
        let major = s
            .parse::<u16>()
            .map_err(|_| ApiVersionError::InvalidMajorVersion)?;

        if major == 0 {
            return Err(ApiVersionError::InvalidMajorVersion);
        }

        Ok(Self::new(major))
    }
}

/// Errors that can occur when parsing an API version.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiVersionError {
    /// The version string is missing the 'v' prefix.
    #[error("API version must start with 'v' (e.g., 'v60.0')")]
    MissingPrefix,

    /// The version string has an invalid format.
    #[error("API version must be in format 'vX.0' (e.g., 'v60.0')")]
    InvalidFormat,

    /// The major version number is invalid.
    #[error("invalid major version number (must be positive integer)")]
    InvalidMajorVersion,
}
#[cfg(test)]
mod tests {
use crate::test_support::Must;
    use super::*;

    // RED PHASE - Write failing tests first

    #[test]
    fn test_new_creates_version() {
        let version = ApiVersion::new(60);
        assert_eq!(version.major(), 60);
    }

    #[test]
    fn test_as_str_format() {
        let version = ApiVersion::new(60);
        assert_eq!(version.as_str(), "v60.0");
    }

    #[test]
    fn test_display_trait() {
        let version = ApiVersion::new(60);
        assert_eq!(format!("{}", version), "v60.0");
    }

    #[test]
    fn test_parse_valid_version() {
        let version: ApiVersion = "v60.0".parse().must();
        assert_eq!(version.major(), 60);
    }

    #[test]
    fn test_parse_missing_prefix() {
        let result = "60.0".parse::<ApiVersion>();
        assert!(matches!(result, Err(ApiVersionError::MissingPrefix)));
    }

    #[test]
    fn test_parse_missing_suffix() {
        let result = "v60".parse::<ApiVersion>();
        assert!(matches!(result, Err(ApiVersionError::InvalidFormat)));
    }

    #[test]
    fn test_parse_wrong_minor_version() {
        let result = "v60.1".parse::<ApiVersion>();
        assert!(matches!(result, Err(ApiVersionError::InvalidFormat)));
    }

    #[test]
    fn test_parse_invalid_major() {
        let result = "vabc.0".parse::<ApiVersion>();
        assert!(matches!(result, Err(ApiVersionError::InvalidMajorVersion)));
    }

    #[test]
    fn test_parse_zero_major() {
        let result = "v0.0".parse::<ApiVersion>();
        assert!(matches!(result, Err(ApiVersionError::InvalidMajorVersion)));
    }

    #[test]
    fn test_ordering() {
        let v58 = ApiVersion::new(58);
        let v59 = ApiVersion::new(59);
        let v60 = ApiVersion::new(60);

        assert!(v58 < v59);
        assert!(v59 < v60);
        assert!(v60 > v58);
    }

    #[test]
    fn test_equality() {
        let v1 = ApiVersion::new(60);
        let v2 = ApiVersion::new(60);
        let v3 = ApiVersion::new(61);

        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }

    #[test]
    fn test_constants() {
        assert_eq!(ApiVersion::V60.major(), 60);
        assert_eq!(ApiVersion::V59.major(), 59);
        assert_eq!(ApiVersion::V58.major(), 58);
        assert_eq!(ApiVersion::V57.major(), 57);
        assert_eq!(ApiVersion::V56.major(), 56);
        assert_eq!(ApiVersion::V55.major(), 55);
        assert_eq!(ApiVersion::MIN_SUPPORTED, ApiVersion::V55);
        assert_eq!(ApiVersion::MAX_TESTED, ApiVersion::V60);
        assert_eq!(ApiVersion::DEFAULT, ApiVersion::V60);
    }

    #[test]
    fn test_latest() {
        let latest = ApiVersion::latest();
        assert_eq!(latest, ApiVersion::V60);
    }

    #[test]
    fn test_support_contract_flags() {
        assert!(ApiVersion::V55.is_supported());
        assert!(ApiVersion::V55.is_tested());
        assert!(ApiVersion::V60.is_tested());
        assert!(ApiVersion::new(61).is_supported());
        assert!(!ApiVersion::new(61).is_tested());
        assert!(!ApiVersion::new(54).is_supported());
    }

    #[test]
    fn test_support_tier_matrix() {
        assert_eq!(
            ApiVersion::new(54).support_tier(),
            ApiVersionSupportTier::Unsupported
        );
        assert_eq!(
            ApiVersion::V55.support_tier(),
            ApiVersionSupportTier::Tested
        );
        assert_eq!(
            ApiVersion::V60.support_tier(),
            ApiVersionSupportTier::Tested
        );
        assert_eq!(
            ApiVersion::new(61).support_tier(),
            ApiVersionSupportTier::SupportedUntested
        );
    }

    #[test]
    fn test_from_str_to_str_roundtrip() {
        let original = "v60.0";
        let version: ApiVersion = original.parse().must();
        assert_eq!(version.as_str(), original);
    }

    #[test]
    fn test_large_version_numbers() {
        let version = ApiVersion::new(999);
        assert_eq!(version.as_str(), "v999.0");
        assert_eq!(version.major(), 999);

        let parsed: ApiVersion = "v999.0".parse().must();
        assert_eq!(parsed, version);
    }

    #[test]
    fn test_single_digit_version() {
        let version = ApiVersion::new(1);
        assert_eq!(version.as_str(), "v1.0");

        let parsed: ApiVersion = "v1.0".parse().must();
        assert_eq!(parsed, version);
    }

    #[test]
    fn test_copy_trait() {
        let v1 = ApiVersion::new(60);
        let v2 = v1; // Copy, not move
        assert_eq!(v1, v2);
        // v1 is still valid after "move"
        assert_eq!(v1.major(), 60);
    }

    #[test]
    fn test_hash_trait() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ApiVersion::new(60));
        set.insert(ApiVersion::new(60)); // Duplicate
        set.insert(ApiVersion::new(61));

        assert_eq!(set.len(), 2); // Only 2 unique versions
    }

    // Property-based tests using proptest
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            // Property 1: Parse -> Display roundtrip
            #[test]
            fn prop_parse_display_roundtrip(major in 1u16..1000u16) {
                let version = ApiVersion::new(major);
                let displayed = format!("{}", version);
                let parsed: ApiVersion = displayed.parse().must();

                prop_assert_eq!(parsed, version);
                prop_assert_eq!(parsed.major(), major);
            }

            // Property 2: as_str() matches Display format
            #[test]
            fn prop_as_str_matches_display(major in 1u16..1000u16) {
                let version = ApiVersion::new(major);
                let as_str = version.as_str();
                let displayed = format!("{}", version);

                prop_assert_eq!(as_str, displayed);
            }

            // Property 3: Valid version strings parse successfully
            #[test]
            fn prop_valid_format_parses(major in 1u16..1000u16) {
                let version_str = format!("v{}.0", major);
                let parsed = version_str.parse::<ApiVersion>();

                prop_assert!(parsed.is_ok());
                prop_assert_eq!(parsed.must().major(), major);
            }

            // Property 4: Missing 'v' prefix always fails
            #[test]
            fn prop_missing_prefix_fails(major in 1u16..1000u16) {
                let version_str = format!("{}.0", major);
                let parsed = version_str.parse::<ApiVersion>();

                prop_assert!(matches!(parsed, Err(ApiVersionError::MissingPrefix)));
            }

            // Property 5: Wrong minor version always fails
            #[test]
            fn prop_wrong_minor_fails(major in 1u16..1000u16, minor in 1u16..100u16) {
                let version_str = format!("v{}.{}", major, minor);
                let parsed = version_str.parse::<ApiVersion>();

                prop_assert!(matches!(parsed, Err(ApiVersionError::InvalidFormat)));
            }

            // Property 6: Ordering is consistent with major version
            #[test]
            fn prop_ordering_consistent(major1 in 1u16..500u16, major2 in 1u16..500u16) {
                let v1 = ApiVersion::new(major1);
                let v2 = ApiVersion::new(major2);

                match major1.cmp(&major2) {
                    std::cmp::Ordering::Less => prop_assert!(v1 < v2),
                    std::cmp::Ordering::Greater => prop_assert!(v1 > v2),
                    std::cmp::Ordering::Equal => prop_assert_eq!(v1, v2),
                }
            }

            // Property 7: Equality is based on major version only
            #[test]
            fn prop_equality_by_major(major in 1u16..1000u16) {
                let v1 = ApiVersion::new(major);
                let v2 = ApiVersion::new(major);

                prop_assert_eq!(v1, v2);
                prop_assert_eq!(v1.major(), v2.major());
            }
        }
    }
}
