#![allow(missing_docs)]
use force::types::{Attributes, SalesforceId};

#[test]
fn test_security_sobject_path_traversal_try_new() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();

    // Attempt path traversal via object type using try_new
    let result = Attributes::try_new("Account/../Secret", &id, "v60.0");

    // Should return error
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("object type contains invalid characters"));
}

#[test]
#[should_panic(expected = "object type contains invalid characters")]
#[allow(deprecated)]
fn test_security_sobject_path_traversal_new_panics() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();

    // Attempt path traversal via object type using deprecated new
    // This should panic
    let _ = Attributes::new("Account/../Secret", &id, "v60.0");
}
