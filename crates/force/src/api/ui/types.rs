//! Shared types for the Salesforce UI API.
//!
//! These types appear across multiple UI API endpoints and are used to
//! represent layout-aware field values, layout modes, and type enumerations.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Represents a single field value with its display value and raw value.
///
/// Every field in a UI API record response is wrapped in this type, which
/// provides both the raw value and a human-readable display string.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldValueRepresentation {
    /// Human-readable display value (e.g., formatted date, picklist label).
    pub display_value: Option<String>,
    /// Raw field value (string, number, boolean, null, or nested record).
    pub value: Option<Value>,
    /// Any additional fields returned by the API not captured above.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Layout type variants for UI API record and layout endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LayoutType {
    /// Compact layout (shown in highlights panel, related list hover cards).
    Compact,
    /// Full page layout.
    Full,
}

/// Interaction mode for UI API layout and record endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Mode {
    /// Create mode (new record form).
    Create,
    /// Edit mode (edit existing record).
    Edit,
    /// View mode (read-only record detail).
    View,
}

impl LayoutType {
    /// Returns the string representation used in API query parameters.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Full => "Full",
        }
    }
}

impl Mode {
    /// Returns the string representation used in API query parameters.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "Create",
            Self::Edit => "Edit",
            Self::View => "View",
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn test_layout_type_as_str() {
        assert_eq!(LayoutType::Compact.as_str(), "Compact");
        assert_eq!(LayoutType::Full.as_str(), "Full");
    }

    #[test]
    fn test_mode_as_str() {
        assert_eq!(Mode::Create.as_str(), "Create");
        assert_eq!(Mode::Edit.as_str(), "Edit");
        assert_eq!(Mode::View.as_str(), "View");
    }

    #[test]
    fn test_field_value_representation_deserialize() {
        let json = r#"{"displayValue":"Test Account","value":"001000000000001AAA","extra_field":"ignored"}"#;
        let fvr: FieldValueRepresentation = serde_json::from_str(json).unwrap();
        assert_eq!(fvr.display_value.as_deref(), Some("Test Account"));
        assert!(fvr.value.is_some());
    }

    #[test]
    fn test_field_value_representation_null_value() {
        let json = r#"{"displayValue":null,"value":null}"#;
        let fvr: FieldValueRepresentation = serde_json::from_str(json).unwrap();
        assert!(fvr.display_value.is_none());
        assert!(fvr.value.is_none());
    }

    #[test]
    fn test_layout_type_equality() {
        assert_eq!(LayoutType::Compact, LayoutType::Compact);
        assert_ne!(LayoutType::Compact, LayoutType::Full);
    }

    #[test]
    fn test_mode_equality() {
        assert_eq!(Mode::View, Mode::View);
        assert_ne!(Mode::View, Mode::Edit);
    }
}
