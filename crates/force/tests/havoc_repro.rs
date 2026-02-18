use force::types::{Attributes, SalesforceId};

#[test]
#[should_panic(expected = "SObject name must start with a letter")]
fn test_attributes_injection_new_panics() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();
    // Havoc: Inject path traversal into type_name
    let malicious_type = "../../../etc/passwd";
    let _attrs = Attributes::new(malicious_type, &id, "v60.0");
}

#[test]
fn test_attributes_injection_try_new_fails() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();
    let malicious_type = "../../../etc/passwd";
    let result = Attributes::try_new(malicious_type, &id, "v60.0");

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    // Verify the error message contains some indication of invalid character
    assert!(
        err_msg.contains("SObject name contains invalid character")
            || err_msg.contains("SObject name must start with a letter")
    );
}

#[test]
#[should_panic(expected = "API version must be in format vXX.X")]
fn test_attributes_api_version_injection_new_panics() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();
    // Havoc: Inject garbage into api_version
    let malicious_version = "v60.0/../../../oops";
    let _attrs = Attributes::new("Account", &id, malicious_version);
}
