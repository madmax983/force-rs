//! Security tests for `SObject` Attributes to prevent injection attacks.

#![allow(clippy::unwrap_used)]

use force::types::{Attributes, SalesforceId};

#[test]
#[should_panic(expected = "invalid SObject attributes")]
fn test_attributes_path_traversal() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();
    let type_name = "../../../etc/passwd";
    let api_version = "v60.0";

    // This should now panic
    let _ = Attributes::new(type_name, &id, api_version);
}

#[test]
#[should_panic(expected = "invalid SObject attributes")]
fn test_attributes_injection_in_version() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();
    let type_name = "Account";
    let api_version = "v60.0/../../../";

    // This should now panic
    let _ = Attributes::new(type_name, &id, api_version);
}

#[test]
fn test_attributes_try_new_returns_error() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();
    let type_name = "Invalid/Name";
    let api_version = "v60.0";

    let result = Attributes::try_new(type_name, &id, api_version);
    assert!(result.is_err());
}
