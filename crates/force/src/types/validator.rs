//! Validation logic for Salesforce types.
//!
//! This module provides strict validation for SObject names and API versions
//! to prevent injection attacks and ensure compliance with Salesforce naming conventions.

use crate::error::{ForceError, Result};

/// Validates an SObject type name.
///
/// Rules:
/// - Must start with a letter.
/// - Must contain only alphanumeric characters and underscores.
/// - Must not end with an underscore.
///
/// # Errors
///
/// Returns `ForceError::InvalidInput` if the name is invalid.
pub fn validate_sobject_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(ForceError::InvalidInput(
            "SObject name cannot be empty".to_string(),
        ));
    }

    let mut chars = name.chars().peekable();

    // check start
    if let Some(first) = chars.peek() {
        if !first.is_ascii_alphabetic() {
            return Err(ForceError::InvalidInput(format!(
                "SObject name must start with a letter: {}",
                name
            )));
        }
    }

    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(ForceError::InvalidInput(format!(
                "SObject name contains invalid character '{}': {}",
                c, name
            )));
        }
    }

    if name.ends_with('_') {
        return Err(ForceError::InvalidInput(format!(
            "SObject name cannot end with an underscore: {}",
            name
        )));
    }

    Ok(())
}

/// Validates an API version string.
///
/// Rules:
/// - Must follow the format `vXX.X` (e.g., `v60.0`).
///
/// # Errors
///
/// Returns `ForceError::InvalidInput` if the version is invalid.
pub fn validate_api_version(version: &str) -> Result<()> {
    if !version.starts_with('v') {
        return Err(ForceError::InvalidInput(format!(
            "API version must start with 'v': {}",
            version
        )));
    }

    let parts: Vec<&str> = version[1..].split('.').collect();
    if parts.len() != 2 {
        return Err(ForceError::InvalidInput(format!(
            "API version must be in format vXX.X: {}",
            version
        )));
    }

    if parts[0].parse::<u32>().is_err() || parts[1].parse::<u32>().is_err() {
        return Err(ForceError::InvalidInput(format!(
            "API version components must be numeric: {}",
            version
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;

    #[test]
    fn test_validate_sobject_name_valid() {
        validate_sobject_name("Account").must();
        validate_sobject_name("Custom_Object__c").must();
        validate_sobject_name("A1").must();
    }

    #[test]
    fn test_validate_sobject_name_invalid() {
        assert!(validate_sobject_name("1Account").is_err());
        assert!(validate_sobject_name("_Account").is_err());
        assert!(validate_sobject_name("Account_").is_err());
        assert!(validate_sobject_name("Account/Other").is_err());
        assert!(validate_sobject_name("../Account").is_err());
    }

    #[test]
    fn test_validate_api_version_valid() {
        validate_api_version("v60.0").must();
        validate_api_version("v58.0").must();
    }

    #[test]
    fn test_validate_api_version_invalid() {
        assert!(validate_api_version("60.0").is_err());
        assert!(validate_api_version("v60").is_err());
        assert!(validate_api_version("v60.").is_err());
        assert!(validate_api_version("v.0").is_err());
        assert!(validate_api_version("v60.0.0").is_err());
        assert!(validate_api_version("vXX.X").is_err());
    }
}
