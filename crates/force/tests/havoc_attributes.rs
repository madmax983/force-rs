use force::types::{Attributes, SalesforceId};

#[test]
#[should_panic(expected = "Invalid Attributes parameters")]
fn havoc_attributes_injection() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();

    // Injecting path traversal
    let bad_type = "Account/../../SecretObject";
    // This should now panic
    let _attrs = Attributes::new(bad_type, &id, "v60.0");
}

#[test]
#[should_panic(expected = "Invalid Attributes parameters")]
fn havoc_attributes_query_injection() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();

    // Injecting query parameters
    let bad_type = "Account?fields=Password__c";
    // This should now panic
    let _attrs = Attributes::new(bad_type, &id, "v60.0");
}
