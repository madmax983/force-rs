//! Validation logic for Salesforce types.

use crate::error::{ForceError, Result};

/// Validates an SObject type name.
///
/// Rules:
/// - Must start with a letter.
/// - Can contain alphanumeric characters and underscores.
/// - Cannot end with an underscore.
pub fn validate_sobject_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(ForceError::InvalidInput(
            "SObject name cannot be empty".into(),
        ));
    }

    let mut chars = name.chars();

    // Check first character
    if let Some(first) = chars.next() {
        if !first.is_ascii_alphabetic() {
            return Err(ForceError::InvalidInput(format!(
                "invalid SObject name format: {} (must start with a letter)",
                name
            )));
        }
    }

    // Check remaining characters
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(ForceError::InvalidInput(format!(
                "invalid SObject name format: {} (contains invalid character '{}')",
                name, c
            )));
        }
    }

    if name.ends_with('_') {
        return Err(ForceError::InvalidInput(
            "SObject name cannot end with an underscore".into(),
        ));
    }

    Ok(())
}

/// Validates an API version string.
///
/// Format: `vXX.X` (e.g., `v60.0`)
pub fn validate_api_version(version: &str) -> Result<()> {
    if !version.starts_with('v') {
        return Err(ForceError::InvalidInput(format!(
            "invalid API version format: {} (must start with 'v')",
            version
        )));
    }

    let rest = &version[1..];
    let parts: Vec<&str> = rest.split('.').collect();

    if parts.len() != 2 {
        return Err(ForceError::InvalidInput(format!(
            "invalid API version format: {} (expected vXX.X)",
            version
        )));
    }

    if parts[0].is_empty() || !parts[0].chars().all(|c| c.is_ascii_digit()) {
        return Err(ForceError::InvalidInput(format!(
            "invalid API version format: {} (major version must be numeric)",
            version
        )));
    }

    if parts[1].is_empty() || !parts[1].chars().all(|c| c.is_ascii_digit()) {
        return Err(ForceError::InvalidInput(format!(
            "invalid API version format: {} (minor version must be numeric)",
            version
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_sobject_name_valid() {
        assert!(validate_sobject_name("Account").is_ok());
        assert!(validate_sobject_name("My_Object__c").is_ok()); // Custom object
        assert!(validate_sobject_name("MyObject").is_ok());
    }

    #[test]
    fn test_validate_sobject_name_invalid() {
        assert!(validate_sobject_name("").is_err());
        assert!(validate_sobject_name("1Account").is_err()); // Starts with number
        assert!(validate_sobject_name("Account_").is_err()); // Ends with underscore
        assert!(validate_sobject_name("Acc..ount").is_err()); // Invalid char
        assert!(validate_sobject_name("Acc/ount").is_err()); // Invalid char (path traversal)
    }

    #[test]
    fn test_validate_api_version_valid() {
        assert!(validate_api_version("v60.0").is_ok());
        assert!(validate_api_version("v58.0").is_ok());
        assert!(validate_api_version("v100.1").is_ok());
    }

    #[test]
    fn test_validate_api_version_invalid() {
        assert!(validate_api_version("60.0").is_err()); // Missing v
        assert!(validate_api_version("v60").is_err()); // Missing decimal
        assert!(validate_api_version("v.0").is_err()); // Missing major
        assert!(validate_api_version("v60.").is_err()); // Missing minor
        assert!(validate_api_version("v60.0/..").is_err()); // Path traversal
        assert!(validate_api_version("v6a.0").is_err()); // Non-digit
    }
}
