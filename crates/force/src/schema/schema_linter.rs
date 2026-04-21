//! Schema Linter for Salesforce SObject Describe metadata.
//!
//! This module provides a utility to evaluate an `SObjectDescribe` against a set of
//! customizable rules to identify schema anti-patterns, technical debt, or potential limits.

use crate::types::describe::SObjectDescribe;

/// Represents the severity of a linter finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintSeverity {
    /// A critical issue that violates platform limits or strong best practices.
    Warning,
    /// A potential improvement or minor code smell.
    Info,
}

/// A finding produced by a `LintRule`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintResult {
    /// The name of the rule that produced this finding.
    pub rule_name: String,
    /// The severity of the finding.
    pub severity: LintSeverity,
    /// A descriptive message explaining the finding.
    pub message: String,
}

/// A trait for implementing schema linting rules.
pub trait LintRule {
    /// Evaluates the given schema describe and returns a list of findings.
    fn evaluate(&self, describe: &SObjectDescribe) -> Vec<LintResult>;
}

/// Warns if an SObject has an excessive number of fields (approaching org limits).
pub struct TooManyFieldsRule {
    /// The maximum allowed number of fields before triggering a warning.
    pub max_fields: usize,
}

impl Default for TooManyFieldsRule {
    fn default() -> Self {
        Self { max_fields: 100 }
    }
}

impl LintRule for TooManyFieldsRule {
    fn evaluate(&self, describe: &SObjectDescribe) -> Vec<LintResult> {
        let count = describe.fields.len();
        if count > self.max_fields {
            vec![LintResult {
                rule_name: "TooManyFields".to_string(),
                severity: LintSeverity::Warning,
                message: format!(
                    "SObject '{}' has {} fields, which exceeds the recommended maximum of {}.",
                    describe.name, count, self.max_fields
                ),
            }]
        } else {
            vec![]
        }
    }
}

/// Warns if there is a custom field where the name does not end in `__c`.
///
/// Note: While Salesforce UI enforces `__c`, API creations or external systems
/// sometimes create metadata inconsistencies. This rule flags them.
pub struct MissingCustomSuffixRule;

impl LintRule for MissingCustomSuffixRule {
    fn evaluate(&self, describe: &SObjectDescribe) -> Vec<LintResult> {
        let mut results = Vec::new();
        for field in &describe.fields {
            if field.custom && !field.name.ends_with("__c") {
                results.push(LintResult {
                    rule_name: "MissingCustomSuffix".to_string(),
                    severity: LintSeverity::Warning,
                    message: format!(
                        "Custom field '{}' in SObject '{}' does not end with '__c'.",
                        field.name, describe.name
                    ),
                });
            }
        }
        results
    }
}

/// The main linter struct that orchestrates the evaluation of multiple rules.
pub struct SchemaLinter {
    rules: Vec<Box<dyn LintRule>>,
}

impl SchemaLinter {
    /// Creates a new linter with no rules.
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Adds a rule to the linter.
    pub fn add_rule<R: LintRule + 'static>(&mut self, rule: R) {
        self.rules.push(Box::new(rule));
    }

    /// Evaluates all configured rules against the given SObject describe.
    #[must_use]
    pub fn lint(&self, describe: &SObjectDescribe) -> Vec<LintResult> {
        // ⚡ Bolt: Pre-allocate capacity tied to the number of rules to reduce heap reallocations.
        let mut results = Vec::with_capacity(self.rules.len());
        for rule in &self.rules {
            results.extend(rule.evaluate(describe));
        }
        results
    }
}

impl Default for SchemaLinter {
    fn default() -> Self {
        let mut linter = Self::new();
        linter.add_rule(TooManyFieldsRule::default());
        linter.add_rule(MissingCustomSuffixRule);
        linter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;
    use serde_json::json;

    fn create_mock_describe(
        name: &str,
        fields_count: usize,
        custom_count: usize,
        missing_suffix: bool,
    ) -> SObjectDescribe {
        let mut fields = Vec::new();
        for i in 0..fields_count {
            let is_custom = i < custom_count;
            let suffix = if is_custom && !(missing_suffix && i == 0) {
                "__c"
            } else {
                ""
            };
            fields.push(json!({
                "name": format!("Field{}{}", i, suffix),
                "type": "string",
                "label": format!("Field {}", i),
                "referenceTo": [],
                "custom": is_custom,
                "calculated": false,
                "nillable": true,
                "defaultedOnCreate": false,
                "createable": true, "autoNumber": false, "aggregatable": true, "byteLength": 18,
                "cascadeDelete": false, "caseSensitive": false,
                "dependentPicklist": false, "deprecatedAndHidden": false,
                "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                "idLookup": false, "length": 18, "nameField": false, "namePointing": false,
                "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
                "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
            }));
        }

        let describe_json = json!({
            "name": name,
            "label": name,
            "custom": true,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": format!("{}s", name), "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": fields
        });
        serde_json::from_value(describe_json).must()
    }

    #[test]
    fn test_too_many_fields_rule() {
        let describe = create_mock_describe("FatObject__c", 105, 5, false);
        let rule = TooManyFieldsRule::default();
        let results = rule.evaluate(&describe);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].rule_name, "TooManyFields");
        assert_eq!(results[0].severity, LintSeverity::Warning);
        assert!(results[0].message.contains("has 105 fields"));
    }

    #[test]
    fn test_missing_custom_suffix_rule() {
        let describe = create_mock_describe("BadObject__c", 5, 2, true);
        let rule = MissingCustomSuffixRule;
        let results = rule.evaluate(&describe);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].rule_name, "MissingCustomSuffix");
        assert_eq!(results[0].severity, LintSeverity::Warning);
        assert!(results[0].message.contains("does not end with '__c'"));
    }

    #[test]
    fn test_schema_linter_default() {
        let describe_clean = create_mock_describe("CleanObject__c", 10, 5, false);
        let describe_dirty = create_mock_describe("GodObject__c", 150, 100, true);

        let linter = SchemaLinter::default();

        let clean_results = linter.lint(&describe_clean);
        assert!(
            clean_results.is_empty(),
            "Clean object should have no findings"
        );

        let dirty_results = linter.lint(&describe_dirty);
        // Should violate TooManyFields (150 > 100) and MissingCustomSuffixRule
        assert_eq!(
            dirty_results.len(),
            2,
            "God object should trigger multiple rules"
        );
        let rule_names: Vec<String> = dirty_results.into_iter().map(|r| r.rule_name).collect();
        assert!(rule_names.contains(&"TooManyFields".to_string()));
        assert!(rule_names.contains(&"MissingCustomSuffix".to_string()));
    }
}
