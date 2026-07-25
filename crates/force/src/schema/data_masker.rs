#[cfg(feature = "schema")]
use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use serde_json::{Map, Value};

/// Redacts sensitive fields in a record based on SObject describe metadata.
#[cfg(feature = "schema")]
pub struct DataMasker {
    mask_char: char,
}

#[cfg(feature = "schema")]
impl DataMasker {
    /// Creates a new `DataMasker` with the default mask character '*'.
    pub fn new() -> Self {
        Self { mask_char: '*' }
    }

    /// Creates a new `DataMasker` with a custom mask character.
    pub fn with_mask_char(mask_char: char) -> Self {
        Self { mask_char }
    }

    /// Masks sensitive fields in a record.
    pub fn mask_record(
        &self,
        describe: &SObjectDescribe,
        mut record: Map<String, Value>,
    ) -> Map<String, Value> {
        for field in &describe.fields {
            if let Some(val) = record.get_mut(&field.name) {
                if self.is_sensitive(field) {
                    *val = self.mask_value(val, field);
                }
            }
        }
        record
    }

    fn is_sensitive(&self, field: &FieldDescribe) -> bool {
        if field.encrypted {
            return true;
        }

        match field.type_ {
            FieldType::Email | FieldType::Phone | FieldType::Encryptedstring => true,
            _ => {
                let lower = field.name.to_lowercase();
                lower.contains("ssn")
                    || lower.contains("socialsecuritynumber")
                    || lower.contains("password")
                    || lower.contains("secret")
            }
        }
    }

    fn mask_value(&self, val: &Value, field: &FieldDescribe) -> Value {
        if val.is_null() {
            return Value::Null;
        }

        let s = match val.as_str() {
            Some(s) => s,
            None => return Value::Null, // For simplicity, if a sensitive field isn't a string, null it out
        };

        if s.is_empty() {
            return Value::String(String::new());
        }

        match field.type_ {
            FieldType::Email => Value::String(format!(
                "{}@{}.com",
                self.mask_char.to_string().repeat(3),
                self.mask_char.to_string().repeat(3)
            )),
            FieldType::Phone => Value::String(format!(
                "({}{}{}) {}{}{}-{}{}{}{}",
                self.mask_char,
                self.mask_char,
                self.mask_char,
                self.mask_char,
                self.mask_char,
                self.mask_char,
                self.mask_char,
                self.mask_char,
                self.mask_char,
                self.mask_char
            )),
            _ => Value::String(self.mask_char.to_string().repeat(s.len().max(3))),
        }
    }
}

#[cfg(feature = "schema")]
impl Default for DataMasker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};
    use serde_json::json;

    #[test]
    fn test_mask_record() {
        let describe = MockSObjectDescribeBuilder::new("Contact")
            .field(MockFieldDescribeBuilder::new("Id", FieldType::Id).build())
            .field(MockFieldDescribeBuilder::new("Name", FieldType::String).build())
            .field(MockFieldDescribeBuilder::new("Email", FieldType::Email).build())
            .field(MockFieldDescribeBuilder::new("Phone", FieldType::Phone).build())
            .field(
                MockFieldDescribeBuilder::new("SocialSecurityNumber__c", FieldType::String).build(),
            )
            .field(MockFieldDescribeBuilder::new("SecretKey__c", FieldType::String).build())
            .field(
                MockFieldDescribeBuilder::new("TopSecret__c", FieldType::String)
                    .encrypted(true)
                    .build(),
            )
            .build();

        let mut record = Map::new();
        record.insert("Id".to_string(), json!("003000000000001"));
        record.insert("Name".to_string(), json!("John Doe"));
        record.insert("Email".to_string(), json!("john.doe@example.com"));
        record.insert("Phone".to_string(), json!("555-123-4567"));
        record.insert("SocialSecurityNumber__c".to_string(), json!("123-45-6789"));
        record.insert("SecretKey__c".to_string(), json!("super_secret"));
        record.insert("TopSecret__c".to_string(), json!("classified"));

        let masker = DataMasker::new();
        let masked = masker.mask_record(&describe, record);

        assert_eq!(masked.get("Id").unwrap(), "003000000000001");
        assert_eq!(masked.get("Name").unwrap(), "John Doe");
        assert_eq!(masked.get("Email").unwrap(), "***@***.com");
        assert_eq!(masked.get("Phone").unwrap(), "(***) ***-****");
        assert_eq!(
            masked.get("SocialSecurityNumber__c").unwrap(),
            "***********"
        );
        assert_eq!(masked.get("SecretKey__c").unwrap(), "************");
        assert_eq!(masked.get("TopSecret__c").unwrap(), "**********");
    }
}
