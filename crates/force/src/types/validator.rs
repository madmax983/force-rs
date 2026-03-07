//! Centralized validation logic for Salesforce domain types.
//!
//! This module provides strict validation functions for SObject names, field names,
//! and other user inputs to prevent injection attacks (SOQL/SOSL/Path Traversal).

use crate::error::ForceError;

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
    if name.is_empty() {
        return Err(ForceError::InvalidInput(
            "SObject name cannot be empty".to_string(),
        ));
    }
    // Strict: [a-zA-Z0-9_]+
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ForceError::InvalidInput(format!(
            "SObject name contains invalid characters: {}",
            name
        )));
    }
    Ok(())
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
    if name.is_empty() {
        return Err(ForceError::InvalidInput(
            "External ID field name cannot be empty".to_string(),
        ));
    }

    // Strict: [a-zA-Z0-9_]+
    // No dots allowed for external ID fields on the object itself.
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ForceError::InvalidInput(format!(
            "External ID field name contains invalid characters: {}",
            name
        )));
    }
    Ok(())
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
            "Field name contains invalid dot usage: {}",
            name
        )));
    }

    for c in name.chars() {
        if !c.is_ascii_alphanumeric() && c != '_' && c != '.' {
            if allow_functions && (c == '(' || c == ')') {
                continue;
            }
            return Err(ForceError::InvalidInput(format!(
                "Field name contains invalid character '{}': {}",
                c, name
            )));
        }
    }

    if allow_functions {
        // Check parenthesis balance
        let mut balance = 0;
        for c in name.chars() {
            if c == '(' {
                balance += 1;
            } else if c == ')' {
                balance -= 1;
            }
            if balance < 0 {
                return Err(ForceError::InvalidInput(format!(
                    "Unbalanced parentheses in field name: {}",
                    name
                )));
            }
        }
        if balance != 0 {
            return Err(ForceError::InvalidInput(format!(
                "Unbalanced parentheses in field name: {}",
                name
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_support::Must;
    use super::*;

    #[test]
    fn test_validate_sobject_name_valid() {
        validate_sobject_name("Account").must();
        validate_sobject_name("Custom_Object__c").must();
        validate_sobject_name("Snippet").must();
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
        validate_field_name("Name").must();
        validate_field_name("Custom_Field__c").must();
        validate_field_name("Parent.Name").must();
        validate_field_name("count(Id)").must();
        validate_field_name("toLabel(StageName)").must();
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
        validate_field_name("Parent.Name").must();
        validate_field_name("Grandparent.Parent.Name").must();
    }

    #[test]
    fn test_validate_external_id_field_valid() {
        validate_external_id_field("ExternalId__c").must();
        validate_external_id_field("Id").must();
    }

    #[test]
    fn test_validate_external_id_field_invalid() {
        assert!(validate_external_id_field("").is_err());
        assert!(validate_external_id_field("Parent.ExternalId__c").is_err()); // No dots
        assert!(validate_external_id_field("Id;").is_err());
    }
}
