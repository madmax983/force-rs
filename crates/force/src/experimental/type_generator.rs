#[cfg(feature = "nova")]
use crate::api::rest::describe::{FieldType, SObjectDescribe};

/// Experimental utility to generate Rust structs from SObject describe metadata.
#[cfg(feature = "nova")]
pub struct StructGenerator;

#[cfg(feature = "nova")]
impl StructGenerator {
    /// Generates a Rust struct definition from an SObject describe result.
    ///
    /// This will generate a struct with `serde` rename attributes to match the
    /// Salesforce API field names, and map the Salesforce types to appropriate
    /// Rust types.
    pub fn generate(describe: &SObjectDescribe) -> String {
        let mut out = String::new();
        out.push_str(&format!("/// {}\n", describe.label));
        out.push_str("#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]\n");
        out.push_str(&format!("pub struct {} {{\n", Self::pascal_case(&describe.name)));

        for field in &describe.fields {
            out.push_str(&format!("    /// {}\n", field.label));
            out.push_str(&format!("    #[serde(rename = \"{}\")]\n", field.name));
            let rust_type = Self::map_type(&field.type_);
            let final_type = if field.nillable {
                format!("Option<{}>", rust_type)
            } else {
                rust_type.to_string()
            };
            out.push_str(&format!("    pub {}: {},\n", Self::snake_case(&field.name), final_type));
        }

        out.push_str("}\n");
        out
    }

    fn pascal_case(s: &str) -> String {
        let mut result = String::new();
        let mut capitalize_next = true;
        for c in s.chars() {
            if c == '_' {
                capitalize_next = true;
            } else if capitalize_next {
                result.push(c.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                result.push(c);
            }
        }
        result
    }

    fn snake_case(s: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = s.chars().collect();
        for i in 0..chars.len() {
            let c = chars[i];
            if c.is_ascii_uppercase() {
                if i > 0 && !chars[i - 1].is_ascii_uppercase() && chars[i-1] != '_' {
                    result.push('_');
                }
                result.push(c.to_ascii_lowercase());
            } else {
                result.push(c);
            }
        }
        if result == "type" {
            result = "type_".to_string();
        }
        result
    }

    fn map_type(ft: &FieldType) -> &'static str {
        match ft {
            FieldType::Boolean => "bool",
            FieldType::Int => "i64",
            FieldType::Double | FieldType::Currency | FieldType::Percent => "f64",
            _ => "String",
        }
    }
}

#[cfg(test)]
#[cfg(feature = "nova")]
mod tests {
    use super::*;

    #[test]
    fn test_snake_case() {
        assert_eq!(StructGenerator::snake_case("Account"), "account");
        assert_eq!(StructGenerator::snake_case("AccountId"), "account_id");
        assert_eq!(StructGenerator::snake_case("IsActive"), "is_active");
        assert_eq!(StructGenerator::snake_case("type"), "type_");
    }

    #[test]
    fn test_pascal_case() {
        assert_eq!(StructGenerator::pascal_case("account"), "Account");
        assert_eq!(StructGenerator::pascal_case("account_id"), "AccountId");
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_generate_struct() {
        use crate::test_support::Must;
        // Construct JSON since SObjectDescribe has many required fields
        let json = r#"{
            "activateable": false,
            "createable": true,
            "custom": false,
            "customSetting": false,
            "deletable": true,
            "deprecatedAndHidden": false,
            "feedEnabled": true,
            "hasSubtypes": false,
            "isSubtype": false,
            "keyPrefix": "001",
            "label": "Account Object",
            "labelPlural": "Accounts",
            "layoutable": true,
            "mergeable": true,
            "mruEnabled": true,
            "name": "Account",
            "queryable": true,
            "replicateable": true,
            "retrieveable": true,
            "searchable": true,
            "triggerable": true,
            "undeletable": true,
            "updateable": true,
            "urls": {
                "sobject": "/services/data/v60.0/sobjects/Account"
            },
            "fields": [
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 18,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": false,
                    "custom": false,
                    "defaultedOnCreate": true,
                    "dependentPicklist": false,
                    "deprecatedAndHidden": false,
                    "digits": 0,
                    "displayLocationInDecimal": false,
                    "encrypted": false,
                    "externalId": false,
                    "filterable": true,
                    "groupable": true,
                    "highScaleNumber": false,
                    "htmlFormatted": false,
                    "idLookup": true,
                    "label": "Account ID",
                    "length": 18,
                    "name": "Id",
                    "nameField": false,
                    "namePointing": false,
                    "nillable": false,
                    "permissionable": false,
                    "polymorphicForeignKey": false,
                    "precision": 0,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "tns:ID",
                    "sortable": true,
                    "type": "id",
                    "unique": false,
                    "updateable": false,
                    "writeRequiresMasterRead": false
                },
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 255,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": true,
                    "custom": false,
                    "defaultedOnCreate": true,
                    "dependentPicklist": false,
                    "deprecatedAndHidden": false,
                    "digits": 0,
                    "displayLocationInDecimal": false,
                    "encrypted": false,
                    "externalId": false,
                    "filterable": true,
                    "groupable": true,
                    "highScaleNumber": false,
                    "htmlFormatted": false,
                    "idLookup": false,
                    "label": "Account Name",
                    "length": 255,
                    "name": "Name",
                    "nameField": true,
                    "namePointing": false,
                    "nillable": true,
                    "permissionable": false,
                    "polymorphicForeignKey": false,
                    "precision": 0,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "xsd:string",
                    "sortable": true,
                    "type": "string",
                    "unique": false,
                    "updateable": true,
                    "writeRequiresMasterRead": false
                },
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 0,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": true,
                    "custom": false,
                    "defaultedOnCreate": true,
                    "dependentPicklist": false,
                    "deprecatedAndHidden": false,
                    "digits": 0,
                    "displayLocationInDecimal": false,
                    "encrypted": false,
                    "externalId": false,
                    "filterable": true,
                    "groupable": true,
                    "highScaleNumber": false,
                    "htmlFormatted": false,
                    "idLookup": false,
                    "label": "Active",
                    "length": 0,
                    "name": "IsActive",
                    "nameField": false,
                    "namePointing": false,
                    "nillable": false,
                    "permissionable": false,
                    "polymorphicForeignKey": false,
                    "precision": 0,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "xsd:boolean",
                    "sortable": true,
                    "type": "boolean",
                    "unique": false,
                    "updateable": true,
                    "writeRequiresMasterRead": false
                }
            ],
            "childRelationships": [],
            "recordTypeInfos": []
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(json).must();

        let result = StructGenerator::generate(&describe);

        assert!(result.contains("/// Account Object"));
        assert!(result.contains("pub struct Account {"));
        assert!(result.contains("pub id: String,"));
        assert!(result.contains("pub name: Option<String>,"));
        assert!(result.contains("pub is_active: bool,"));
        assert!(result.contains("#[serde(rename = \"Id\")]"));
    }
}
