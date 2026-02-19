//! Regression tests for `DynamicSObject` issues.

#![allow(clippy::unwrap_used)]

use force::types::{Attributes, DynamicSObject, SalesforceId};
use serde::{Serialize, Serializer};

struct FailSerialize;

impl Serialize for FailSerialize {
    fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(serde::ser::Error::custom("Always fail"))
    }
}

#[test]
#[should_panic(expected = "serialization failed")]
fn test_dynamic_sobject_silent_failure_is_now_panic() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();
    let attrs = Attributes::new("Account", &id, "v60.0");
    let mut obj = DynamicSObject::new(attrs);

    // This will fail to serialize internally and should panic now
    obj.set_field("Fail", FailSerialize);
}

#[test]
fn test_dynamic_sobject_try_set_field_returns_error() {
    let id = SalesforceId::new("001000000000001AAA").unwrap();
    let attrs = Attributes::new("Account", &id, "v60.0");
    let mut obj = DynamicSObject::new(attrs);

    // This will fail to serialize internally
    let result = obj.try_set_field("Fail", FailSerialize);

    assert!(result.is_err());
    assert_eq!(obj.field_count(), 0);
}
