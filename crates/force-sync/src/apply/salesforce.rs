//! Salesforce REST apply helpers.

use crate::error::ForceSyncError;
use force::{
    api::RestOperation,
    api::SoqlQueryBuilder,
    api::bulk::{JobOperation, SmartIngestResult},
    auth::Authenticator,
    client::ForceClient,
    error::{ForceError, HttpError},
    types::DynamicSObject,
    types::SalesforceId,
};
use futures::stream;
use serde::Serialize;
use serde_json::Value;

/// Result of applying a REST upsert to Salesforce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestApplyResult {
    /// Salesforce ID returned by the API, if available.
    pub salesforce_id: Option<SalesforceId>,
    /// Whether the upsert created a new record.
    pub created: bool,
}

/// Minimal Salesforce REST applier for force-sync lanes.
#[derive(Debug, Clone)]
pub struct SalesforceApplier<A: Authenticator> {
    client: ForceClient<A>,
}

impl<A: Authenticator> SalesforceApplier<A> {
    /// Creates a new applier from an authenticated `force` client.
    #[must_use]
    pub const fn new(client: ForceClient<A>) -> Self {
        Self { client }
    }

    /// Applies an external-ID upsert using the idempotent REST lane.
    ///
    /// # Errors
    ///
    /// Returns a retryable or permanent apply error if the request fails.
    pub async fn apply_rest_upsert(
        &self,
        sobject: &str,
        external_id_field: &str,
        external_id_value: &str,
        payload: &Value,
    ) -> crate::error::Result<RestApplyResult> {
        let rest = self.client.rest();

        match rest
            .upsert_idempotent(sobject, external_id_field, external_id_value, payload)
            .await
        {
            Ok(response) => Ok(RestApplyResult {
                salesforce_id: Some(response.id),
                created: response.created,
            }),
            Err(error) if is_missing_id_upsert_update(&error) => Ok(RestApplyResult {
                salesforce_id: Some(
                    self.resolve_salesforce_id(sobject, external_id_field, external_id_value)
                        .await?,
                ),
                created: false,
            }),
            Err(error) => Err(classify_force_error(error)),
        }
    }

    /// Applies a REST delete using a known Salesforce record ID.
    ///
    /// # Errors
    ///
    /// Returns a retryable or permanent apply error if the request fails.
    pub async fn apply_rest_delete(
        &self,
        sobject: &str,
        salesforce_id: &SalesforceId,
    ) -> crate::error::Result<()> {
        match self.client.rest().delete(sobject, salesforce_id).await {
            Ok(_) => Ok(()),
            Err(error) => {
                if matches!(
                    error,
                    ForceError::Http(HttpError::StatusError {
                        status_code: 404,
                        ..
                    })
                ) {
                    Ok(())
                } else {
                    Err(classify_force_error(error))
                }
            }
        }
    }

    /// Applies a bulk upsert using an external ID field and configured batch size.
    ///
    /// # Errors
    ///
    /// Returns a retryable or permanent apply error if the bulk job fails.
    pub async fn apply_bulk_upsert<T>(
        &self,
        sobject: &str,
        external_id_field: &str,
        batch_size: usize,
        records: Vec<T>,
    ) -> crate::error::Result<SmartIngestResult>
    where
        T: Serialize + Send + Sync,
    {
        self.client
            .bulk()
            .smart_ingest(sobject, JobOperation::Upsert)
            .external_id_field(external_id_field)
            .batch_size(batch_size)
            .execute_stream(stream::iter(records))
            .await
            .map_err(classify_force_error)
    }

    async fn resolve_salesforce_id(
        &self,
        sobject: &str,
        external_id_field: &str,
        external_id_value: &str,
    ) -> crate::error::Result<SalesforceId> {
        let query = SoqlQueryBuilder::new()
            .select(&["Id"])
            .from(sobject)
            .where_eq(external_id_field, external_id_value)
            .limit(1)
            .build();

        let result = self
            .client
            .rest()
            .query::<DynamicSObject>(&query)
            .await
            .map_err(classify_force_error)?;

        let Some(record) = result.records.into_iter().next() else {
            return Err(missing_follow_up_id_error(
                sobject,
                external_id_field,
                external_id_value,
            ));
        };

        let Some(id) = record.get_field_as::<String>("Id").map_err(|error| {
            ForceSyncError::ApplyPermanent(ForceError::InvalidInput(format!(
                "invalid Salesforce query result: {error}"
            )))
        })?
        else {
            return Err(missing_follow_up_id_error(
                sobject,
                external_id_field,
                external_id_value,
            ));
        };

        SalesforceId::new(&id).map_err(|error| {
            ForceSyncError::ApplyPermanent(ForceError::InvalidInput(format!(
                "invalid Salesforce query result ID: {error}"
            )))
        })
    }
}

fn is_missing_id_upsert_update(error: &ForceError) -> bool {
    matches!(
        error,
        ForceError::NotImplemented(message)
            if message.contains("204") && message.contains("record ID")
    )
}

const fn classify_force_error(error: ForceError) -> ForceSyncError {
    if is_retryable_force_error(&error) {
        ForceSyncError::ApplyRetryable(error)
    } else {
        ForceSyncError::ApplyPermanent(error)
    }
}

fn missing_follow_up_id_error(
    sobject: &str,
    external_id_field: &str,
    external_id_value: &str,
) -> ForceSyncError {
    ForceSyncError::ApplyPermanent(ForceError::InvalidInput(format!(
        "upsert updated existing {sobject} via {external_id_field}={external_id_value}, but follow-up lookup did not return a Salesforce Id"
    )))
}

const fn is_retryable_force_error(error: &ForceError) -> bool {
    match error {
        ForceError::Http(HttpError::StatusError { status_code, .. }) => {
            *status_code == 408 || *status_code == 409 || *status_code == 429 || *status_code >= 500
        }
        ForceError::Http(
            HttpError::RateLimitExceeded { .. }
            | HttpError::Timeout { .. }
            | HttpError::RequestFailed(_),
        ) => true,
        _ => false,
    }
}
