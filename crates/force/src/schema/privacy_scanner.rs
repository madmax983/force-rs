use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};

/// Represents the category of a privacy risk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskCategory {
    /// Personally Identifiable Information (e.g., Name, Email, Phone, SSN).
    Pii,
    /// Financial Information (e.g., Credit Card, Revenue, Salary).
    Financial,
    /// Security / Authentication Information (e.g., Password, Token, Secret).
    Security,
}

/// Represents the severity of a privacy risk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskLevel {
    /// High risk: clear PII, financial, or security data, especially if unencrypted.
    High,
    /// Medium risk: potential PII or data that might be sensitive depending on context.
    Medium,
    /// Low risk: standard fields that are generally safe but warrant a review.
    Low,
}

/// A privacy risk identified in a field.
#[derive(Debug, Clone, PartialEq)]
pub struct PrivacyRisk<'a> {
    /// The field that triggered the risk.
    pub field: &'a FieldDescribe,
    /// The category of the risk.
    pub category: RiskCategory,
    /// The severity of the risk.
    pub level: RiskLevel,
    /// The reason this field was flagged.
    pub reason: &'static str,
}

/// A scanner that analyzes an SObject schema for potential privacy and compliance risks.
#[derive(Debug, Default)]
pub struct PrivacyScanner;

impl PrivacyScanner {
    /// Creates a new PrivacyScanner.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Scans the provided SObject describe payload for privacy risks.
    #[must_use]
    pub fn scan<'a>(&self, describe: &'a SObjectDescribe) -> Vec<PrivacyRisk<'a>> {
        let mut risks = Vec::new();

        for field in &describe.fields {
            if let Some(risk) = Self::analyze_field(field) {
                risks.push(risk);
            }
        }

        risks
    }

    fn analyze_field(field: &FieldDescribe) -> Option<PrivacyRisk<'_>> {
        let name_lower = field.name.to_ascii_lowercase();

        // 1. Security / Secrets
        if name_lower.contains("password")
            || name_lower.contains("secret")
            || name_lower.contains("token")
        {
            return Some(PrivacyRisk {
                field,
                category: RiskCategory::Security,
                level: if field.encrypted {
                    RiskLevel::Medium
                } else {
                    RiskLevel::High
                },
                reason: "Field name suggests it contains security credentials or secrets.",
            });
        }

        // 2. Financial / PCI
        if name_lower.contains("creditcard")
            || name_lower.contains("ssn")
            || name_lower.contains("socialsecurity")
        {
            return Some(PrivacyRisk {
                field,
                category: RiskCategory::Financial,
                level: if field.encrypted {
                    RiskLevel::Medium
                } else {
                    RiskLevel::High
                },
                reason: "Field name suggests it contains highly sensitive financial or government ID data.",
            });
        }

        if name_lower.contains("salary")
            || name_lower.contains("revenue")
            || name_lower.contains("income")
        {
            return Some(PrivacyRisk {
                field,
                category: RiskCategory::Financial,
                level: RiskLevel::Medium,
                reason: "Field name suggests it contains financial data.",
            });
        }

        // 3. PII (Types & Names)
        match field.type_ {
            FieldType::Email => {
                return Some(PrivacyRisk {
                    field,
                    category: RiskCategory::Pii,
                    level: RiskLevel::High,
                    reason: "Field is of type Email.",
                });
            }
            FieldType::Phone => {
                return Some(PrivacyRisk {
                    field,
                    category: RiskCategory::Pii,
                    level: RiskLevel::Medium,
                    reason: "Field is of type Phone.",
                });
            }
            _ => {}
        }

        if name_lower.contains("firstname")
            || name_lower.contains("lastname")
            || name_lower.contains("birthdate")
            || name_lower.contains("dob")
        {
            return Some(PrivacyRisk {
                field,
                category: RiskCategory::Pii,
                level: RiskLevel::Medium,
                reason: "Field name suggests it contains Personally Identifiable Information (PII).",
            });
        }

        // 4. Catch-all for explicitly encrypted fields not caught by heuristics
        if field.encrypted {
            return Some(PrivacyRisk {
                field,
                category: RiskCategory::Security,
                level: RiskLevel::Low,
                reason: "Field is marked as encrypted, indicating it contains sensitive data.",
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use serde_json::json;

    fn mock_field(name: &str, field_type: &str, encrypted: bool) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "encrypted": encrypted,
            "createable": true, "autoNumber": false, "calculated": false,
            "aggregatable": true, "byteLength": 255, "cascadeDelete": false,
            "caseSensitive": false, "custom": false, "defaultedOnCreate": false,
            "dependentPicklist": false, "deprecatedAndHidden": false, "digits": 0,
            "displayLocationInDecimal": false, "externalId": false, "filterable": true,
            "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": false, "length": 255, "nameField": false, "namePointing": false,
            "nillable": true, "permissionable": false, "polymorphicForeignKey": false,
            "precision": 0, "queryByDistance": false, "restrictedDelete": false,
            "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
            "sortable": true, "unique": false, "updateable": true,
            "writeRequiresMasterRead": false
        })
    }

    fn create_mock_describe(fields_json: &serde_json::Value) -> SObjectDescribe {
        let describe_json = json!({
            "name": "Contact",
            "label": "Contact",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "003", "labelPlural": "Contacts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": fields_json.clone()
        });
        serde_json::from_value(describe_json).must()
    }

    #[test]
    fn test_privacy_scanner_pii() {
        let describe = create_mock_describe(&json!([
            mock_field("Email", "email", false),
            mock_field("Phone", "phone", false),
            mock_field("FirstName", "string", false),
            mock_field("SafeField", "string", false)
        ]));

        let scanner = PrivacyScanner::new();
        let risks = scanner.scan(&describe);

        assert_eq!(risks.len(), 3);
        assert_eq!(risks[0].field.name, "Email");
        assert_eq!(risks[0].category, RiskCategory::Pii);
        assert_eq!(risks[0].level, RiskLevel::High);

        assert_eq!(risks[1].field.name, "Phone");
        assert_eq!(risks[1].category, RiskCategory::Pii);
        assert_eq!(risks[1].level, RiskLevel::Medium);

        assert_eq!(risks[2].field.name, "FirstName");
        assert_eq!(risks[2].category, RiskCategory::Pii);
        assert_eq!(risks[2].level, RiskLevel::Medium);
    }

    #[test]
    fn test_privacy_scanner_security() {
        let describe = create_mock_describe(&json!([
            mock_field("Password__c", "string", false),
            mock_field("ClientSecret", "string", true),
            mock_field("OtherEncrypted__c", "string", true)
        ]));

        let scanner = PrivacyScanner::new();
        let risks = scanner.scan(&describe);

        assert_eq!(risks.len(), 3);

        // Unencrypted password -> High Risk
        assert_eq!(risks[0].field.name, "Password__c");
        assert_eq!(risks[0].category, RiskCategory::Security);
        assert_eq!(risks[0].level, RiskLevel::High);

        // Encrypted secret -> Medium Risk
        assert_eq!(risks[1].field.name, "ClientSecret");
        assert_eq!(risks[1].category, RiskCategory::Security);
        assert_eq!(risks[1].level, RiskLevel::Medium);

        // Generic encrypted field -> Low Risk
        assert_eq!(risks[2].field.name, "OtherEncrypted__c");
        assert_eq!(risks[2].category, RiskCategory::Security);
        assert_eq!(risks[2].level, RiskLevel::Low);
    }

    #[test]
    fn test_privacy_scanner_financial() {
        let describe = create_mock_describe(&json!([
            mock_field("CreditCardNumber", "string", false),
            mock_field("AnnualRevenue", "currency", false)
        ]));

        let scanner = PrivacyScanner::new();
        let risks = scanner.scan(&describe);

        assert_eq!(risks.len(), 2);

        assert_eq!(risks[0].field.name, "CreditCardNumber");
        assert_eq!(risks[0].category, RiskCategory::Financial);
        assert_eq!(risks[0].level, RiskLevel::High);

        assert_eq!(risks[1].field.name, "AnnualRevenue");
        assert_eq!(risks[1].category, RiskCategory::Financial);
        assert_eq!(risks[1].level, RiskLevel::Medium);
    }
}
