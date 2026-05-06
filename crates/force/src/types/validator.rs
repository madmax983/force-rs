//! Centralized validation logic for Salesforce domain types.
//!
//! This module provides strict validation functions for SObject names, field names,
//! and other user inputs to prevent injection attacks (SOQL/SOSL/Path Traversal).

use crate::error::ForceError;

/// Validates that a string contains only alphanumeric characters and underscores.
///
/// Shared logic for SObject names, external ID fields, and any other identifier
/// that must be a strict `[a-zA-Z0-9_]+` pattern.
pub fn validate_identifier(name: &str, label: &str) -> Result<(), ForceError> {
    if name.is_empty() {
        return Err(ForceError::InvalidInput(format!("{label} cannot be empty")));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ForceError::InvalidInput(format!(
            "{label} contains invalid characters: {name}"
        )));
    }
    Ok(())
}

/// Validates an SObject name (e.g., "Account", "Custom__c").
///
/// # Rules
/// - Must not be empty.
/// - Must contain only alphanumeric characters and underscores.
///
/// # Security
///
/// This prevents path traversal and injection attacks when SObject names are used
/// in URLs or queries.
pub fn validate_sobject_name(name: &str) -> Result<(), ForceError> {
    validate_identifier(name, "SObject name")
}

/// Validates a field name or path (e.g., "Name", "Parent.Name").
///
/// # Rules
/// - Must not be empty.
/// - Must contain only alphanumeric characters, underscores, and dots.
/// - Allows parentheses for function calls if `allow_functions` is true.
/// - Checks for balanced parentheses.
///
/// # Security
///
/// This prevents SOQL injection when field names are interpolated into queries.
pub fn validate_field_name(name: &str) -> Result<(), ForceError> {
    validate_field_name_internal(name, true)
}

/// Validates an external ID field name (e.g., "ExternalId__c").
///
/// # Rules
/// - Must not be empty.
/// - Must contain only alphanumeric characters and underscores.
/// - Dots are NOT allowed (external ID fields are on the object itself).
///
/// # Security
///
/// This ensures that the external ID field is a valid identifier on the object,
/// preventing path manipulation in upsert requests.
pub fn validate_external_id_field(name: &str) -> Result<(), ForceError> {
    validate_identifier(name, "External ID field name")
}

fn validate_field_name_internal(name: &str, allow_functions: bool) -> Result<(), ForceError> {
    if name.is_empty() {
        return Err(ForceError::InvalidInput(
            "Field name cannot be empty".to_string(),
        ));
    }

    // Check for invalid dot usage (path traversal / malformed paths)
    if name.starts_with('.') || name.ends_with('.') || name.contains("..") {
        return Err(ForceError::InvalidInput(format!(
            "Field name contains invalid dot usage: {name}"
        )));
    }

    // Single pass: validate characters and track parenthesis balance
    let mut balance: i32 = 0;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            continue;
        }
        if allow_functions && (c == '(' || c == ')') {
            if c == '(' {
                balance = balance.checked_add(1).ok_or_else(|| {
                    ForceError::InvalidInput(format!(
                        "Parentheses nesting too deep in field name: {name}"
                    ))
                })?;
            } else {
                balance = balance.checked_sub(1).ok_or_else(|| {
                    ForceError::InvalidInput(format!(
                        "Unbalanced parentheses in field name: {name}"
                    ))
                })?;
            }
            if balance < 0 {
                return Err(ForceError::InvalidInput(format!(
                    "Unbalanced parentheses in field name: {name}"
                )));
            }
            continue;
        }
        return Err(ForceError::InvalidInput(format!(
            "Field name contains invalid character '{c}': {name}"
        )));
    }

    if allow_functions && balance != 0 {
        return Err(ForceError::InvalidInput(format!(
            "Unbalanced parentheses in field name: {name}"
        )));
    }

    Ok(())
}

/// Validates an API URL path.
///
/// # Rules
/// - Must not be empty.
/// - Must not contain path traversal characters (`..`, `//`).
/// - Must not start with a schema (e.g., `http://`, `https://`).
///
/// # Security
///
/// This prevents SSRF and path traversal attacks when user inputs are used
/// directly in composite requests or dynamic URLs.
pub fn validate_url_path(path: &str) -> Result<(), ForceError> {
    if path.is_empty() {
        return Err(ForceError::InvalidInput(
            "URL path cannot be empty".to_string(),
        ));
    }
    if path.starts_with("http://") || path.starts_with("https://") {
        return Err(ForceError::InvalidInput(format!(
            "URL path must be relative, but absolute URL was provided: {path}"
        )));
    }

    // Parse the URL to extract the path without resolving/normalizing it,
    // so we can catch explicit ".." components in the raw path.
    let base =
        url::Url::parse("http://localhost").unwrap_or_else(|_| unreachable!("valid base url"));
    let _parsed = base
        .join(path)
        .map_err(|_| ForceError::InvalidInput(format!("Invalid URL path: {path}")))?;

    // Since `Url::join` resolves `..` (e.g., `/a/b/..` -> `/a/`), checking `parsed.path()`
    // directly won't catch `..`. So we need to look at the raw input string,
    // but only the path part (before `?` or `#`).
    let path_only = path.split(['?', '#']).next().unwrap_or(path);
    let decoded_path = percent_encoding::percent_decode_str(path_only).decode_utf8_lossy();

    if decoded_path.contains("..") || decoded_path.contains("//") {
        return Err(ForceError::InvalidInput(format!(
            "URL path contains invalid path traversal characters: {path}"
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
        assert!(validate_sobject_name("Custom_Object__c").is_ok());
        assert!(validate_sobject_name("Snippet").is_ok());
    }

    #[test]
    fn test_validate_sobject_name_invalid() {
        assert!(validate_sobject_name("").is_err());
        assert!(validate_sobject_name("Account; DROP TABLE").is_err());
        assert!(validate_sobject_name("Account/Test").is_err());
        assert!(validate_sobject_name("Account.Field").is_err()); // Dots not allowed in SObject name
    }

    #[test]
    fn test_validate_field_name_valid() {
        assert!(validate_field_name("Name").is_ok());
        assert!(validate_field_name("Custom_Field__c").is_ok());
        assert!(validate_field_name("Parent.Name").is_ok());
        assert!(validate_field_name("count(Id)").is_ok());
        assert!(validate_field_name("toLabel(StageName)").is_ok());
    }

    #[test]
    fn test_validate_field_name_invalid() {
        assert!(validate_field_name("").is_err());
        assert!(validate_field_name("Name; DROP").is_err());
        assert!(validate_field_name("Name--").is_err());
        assert!(validate_field_name("count(Id").is_err());
        assert!(validate_field_name("count)Id(").is_err());
    }

    #[test]
    fn test_validate_field_name_edge_cases_dots() {
        // Leading/trailing dots
        assert!(validate_field_name(".Name").is_err());
        assert!(validate_field_name("Name.").is_err());
        // Consecutive dots
        assert!(validate_field_name("Parent..Name").is_err());
        assert!(validate_field_name("..").is_err());

        // Valid dots
        assert!(validate_field_name("Parent.Name").is_ok());
        assert!(validate_field_name("Grandparent.Parent.Name").is_ok());
    }

    #[test]
    fn test_validate_external_id_field_valid() {
        assert!(validate_external_id_field("ExternalId__c").is_ok());
        assert!(validate_external_id_field("Id").is_ok());
    }

    #[test]
    fn test_validate_external_id_field_invalid() {
        assert!(validate_external_id_field("").is_err());
        assert!(validate_external_id_field("Parent.ExternalId__c").is_err()); // No dots
        assert!(validate_external_id_field("Id;").is_err());
    }

    #[test]
    fn test_validate_url_path_valid() {
        assert!(validate_url_path("query?q=SELECT+Id+FROM+Account").is_ok());
        assert!(validate_url_path("sobjects/Account/001000000000000AAA").is_ok());
        // '..' in query string should be allowed
        assert!(validate_url_path("query?q=SELECT+Name+FROM+Account+WHERE+Name='..'").is_ok());
        assert!(validate_url_path("query?q=SELECT+Name+FROM+Account+WHERE+Name='//'").is_ok());
    }

    #[test]
    fn test_validate_url_path_invalid() {
        assert!(validate_url_path("").is_err());
        assert!(validate_url_path("http://evil.com").is_err());
        assert!(validate_url_path("https://evil.com").is_err());
        assert!(validate_url_path("sobjects/Account/001000000000000AAA/..").is_err());
        assert!(validate_url_path("sobjects/Account/../../Contact").is_err());
        assert!(validate_url_path("sobjects//Account").is_err());
        assert!(validate_url_path("../../../etc/passwd").is_err());
        assert!(validate_url_path("sobjects/Account/%2e%2e/Contact").is_err());
        assert!(validate_url_path("sobjects/Account/%2E%2E/Contact").is_err());
        assert!(validate_url_path("%2e%2e/%2e%2e/%2e%2e/etc/passwd").is_err());
        assert!(validate_url_path("%2f%2f").is_err());
    }
}

#[cfg(test)]
mod havoc_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn havoc_never_panics_on_any_string(s in ".*") {
            let _ = validate_field_name(&s);
            let _ = validate_sobject_name(&s);
            let _ = validate_external_id_field(&s);
            let _ = validate_identifier(&s, "test");
            let _ = validate_url_path(&s);
        }
    }
}
