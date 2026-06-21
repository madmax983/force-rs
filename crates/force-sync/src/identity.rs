//! Canonical identity types for synced records.

use crate::error::ForceSyncError;
use crate::error::Result;

/// Canonical identity for a synced record.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyncKey {
    tenant: String,
    object_name: String,
    external_id: String,
}

impl SyncKey {
    /// Creates a canonical identity from non-empty tenant, object, and external ID parts.
    ///
    /// # Errors
    ///
    /// Returns an error when any part is empty.
    pub fn new(
        tenant: impl Into<String>,
        object_name: impl Into<String>,
        external_id: impl Into<String>,
    ) -> Result<Self> {
        let tenant = tenant.into();
        let object_name = object_name.into();
        let external_id = external_id.into();

        if tenant.is_empty() {
            return Err(ForceSyncError::EmptySyncKeyPart { part: "tenant" });
        }

        if object_name.is_empty() {
            return Err(ForceSyncError::EmptySyncKeyPart {
                part: "object_name",
            });
        }

        if external_id.is_empty() {
            return Err(ForceSyncError::EmptySyncKeyPart {
                part: "external_id",
            });
        }

        Ok(Self {
            tenant,
            object_name,
            external_id,
        })
    }

    /// Returns the tenant identifier.
    #[must_use]
    pub fn tenant(&self) -> &str {
        &self.tenant
    }

    /// Returns the synced object name.
    #[must_use]
    pub fn object_name(&self) -> &str {
        &self.object_name
    }

    /// Returns the canonical external ID.
    #[must_use]
    pub fn external_id(&self) -> &str {
        &self.external_id
    }
}

#[cfg(test)]
mod tests {
    use super::SyncKey;

    #[test]
    fn sync_key_requires_non_empty_parts() {
        let Err(err) = SyncKey::new("", "Account", "abc") else {
            panic!("empty tenant should fail");
        };

        assert!(err.to_string().contains("tenant"));
    }
}
