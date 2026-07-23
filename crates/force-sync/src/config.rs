//! Object-level sync configuration types.

use std::collections::BTreeMap;

/// Declares which system owns a field or field group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// Salesforce is authoritative.
    Salesforce,
    /// Postgres is authoritative.
    Postgres,
    /// Either side may update the field and conflicts must be resolved elsewhere.
    Shared,
}

/// Conflict resolution policy for an object sync definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConflictPolicy {
    /// Use explicit field ownership rules.
    FieldOwnership,
    /// Record the conflict for later manual resolution.
    #[default]
    ManualReview,
}

/// Transport cutovers for the planner lanes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneThresholds {
    rest_max_batch_size: usize,
    bulk_min_batch_size: usize,
}

impl LaneThresholds {
    /// Returns the maximum batch size before the planner should avoid REST.
    #[must_use]
    pub const fn rest_max_batch_size(&self) -> usize {
        self.rest_max_batch_size
    }

    /// Returns the minimum batch size that should prefer bulk transport.
    #[must_use]
    pub const fn bulk_min_batch_size(&self) -> usize {
        self.bulk_min_batch_size
    }
}

impl Default for LaneThresholds {
    fn default() -> Self {
        Self {
            rest_max_batch_size: 25,
            bulk_min_batch_size: 500,
        }
    }
}

/// Object-level sync configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectSync {
    object_name: String,
    external_id_field: Option<String>,
    conflict_policy: ConflictPolicy,
    lane_thresholds: LaneThresholds,
    field_ownership: BTreeMap<String, Owner>,
}

impl ObjectSync {
    /// Creates a new object sync definition.
    #[must_use]
    pub fn new(object_name: impl Into<String>) -> Self {
        Self {
            object_name: object_name.into(),
            external_id_field: None,
            conflict_policy: ConflictPolicy::default(),
            lane_thresholds: LaneThresholds::default(),
            field_ownership: BTreeMap::new(),
        }
    }

    /// Sets the Salesforce external ID field used for canonical identity.
    #[must_use]
    pub fn external_id(mut self, external_id_field: impl Into<String>) -> Self {
        self.external_id_field = Some(external_id_field.into());
        self
    }

    /// Marks a field as owned by one side or shared.
    #[must_use]
    pub fn field_owner(mut self, field_name: impl Into<String>, owner: Owner) -> Self {
        self.field_ownership.insert(field_name.into(), owner);
        self
    }

    /// Returns the synced Salesforce object name.
    #[must_use]
    pub fn object_name(&self) -> &str {
        &self.object_name
    }

    /// Returns the configured external ID field, if one is set.
    #[must_use]
    pub fn external_id_field(&self) -> Option<&str> {
        self.external_id_field.as_deref()
    }

    /// Returns the conflict policy for the object.
    #[must_use]
    pub const fn conflict_policy(&self) -> &ConflictPolicy {
        &self.conflict_policy
    }

    /// Returns the planner thresholds for the object.
    #[must_use]
    pub const fn lane_thresholds(&self) -> &LaneThresholds {
        &self.lane_thresholds
    }

    /// Returns the ownership rule for a field, if one is configured.
    #[must_use]
    pub fn field_owner_for(&self, field_name: &str) -> Option<Owner> {
        self.field_ownership.get(field_name).copied()
    }

    /// Returns the configured field ownership rules.
    #[must_use]
    pub const fn field_ownership(&self) -> &BTreeMap<String, Owner> {
        &self.field_ownership
    }
}

#[cfg(test)]
mod tests {
    use super::{ObjectSync, Owner};

    #[test]
    fn object_sync_external_id_builder_stores_config() {
        let config = ObjectSync::new("Account").external_id("External_Id__c");

        assert_eq!(config.object_name(), "Account");
        assert_eq!(config.external_id_field(), Some("External_Id__c"));
    }

    #[test]
    fn object_sync_field_owner_builder_stores_rules() {
        let config = ObjectSync::new("Account").field_owner("Name", Owner::Postgres);

        assert_eq!(config.field_owner_for("Name"), Some(Owner::Postgres));
        assert_eq!(config.field_ownership().len(), 1);
    }

    #[test]
    fn test_lane_thresholds_defaults() {
        let thresholds = super::LaneThresholds::default();
        assert_eq!(thresholds.rest_max_batch_size(), 25);
        assert_eq!(thresholds.bulk_min_batch_size(), 500);
    }
}
