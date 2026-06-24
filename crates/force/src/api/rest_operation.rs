//! Shared REST operation trait for Salesforce API handlers.
//!
//! This module defines the [`RestOperation`] trait which provides default
//! implementations for CRUD, Query, and Describe operations. Both `RestHandler`
//! (REST API) and `ToolingHandler` (Tooling API) implement this trait, differing
//! only in their path prefix:
//!
//! - **REST API**: `sobjects/Account` (no prefix)
//! - **Tooling API**: `tooling/sobjects/Account` (prefix: `"tooling"`)
//!
//! Implementors need only supply [`session()`](RestOperation::session) and
//! [`path_prefix()`](RestOperation::path_prefix); all HTTP operations are
//! provided as default methods.

use crate::auth::Authenticator;
use crate::error::{ForceError, Result};
use crate::session::Session;
use crate::types::common::{CreateResponse, DeleteResponse, UpdateResponse, UpsertResponse};
use crate::types::describe::{GlobalDescribe, SObjectDescribe};
use crate::types::validator::{validate_external_id_field, validate_sobject_name};
use crate::types::{QueryResult, SalesforceId};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use std::sync::Arc;

/// Custom encode set for External ID values in upsert paths.
/// Preserves safe path characters (`-`, `_`, `.`, `~`) as per RFC 3986.
const UPSERT_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

const MAX_QUERY_INPUT_BYTES: usize = 100_000;

/// Trait providing default REST operation implementations for Salesforce API handlers.
///
/// Both `RestHandler` and `ToolingHandler` implement this trait. The only difference
/// between handlers is the value returned by [`path_prefix()`](Self::path_prefix),
/// which controls whether requests are routed to the standard REST API or the
/// Tooling API.
///
/// # URL Construction
///
/// All methods construct URLs using the chain:
///
/// 1. [`resolve_api_path()`](Self::resolve_api_path) prepends the path prefix
///    (e.g., `"sobjects/Account"` becomes `"tooling/sobjects/Account"` for
///    the Tooling API).
/// 2. [`Session::resolve_url()`] constructs the full URL:
///    `{instance_url}/services/data/{api_version}/{path}`.
///
/// # Examples
///
/// ```ignore
/// // REST API (prefix = "")
/// client.rest().create("Account", &data).await?;
/// // => POST {instance}/services/data/v60.0/sobjects/Account
///
/// // Tooling API (prefix = "tooling")
/// client.tooling().create("ApexClass", &data).await?;
/// // => POST {instance}/services/data/v60.0/tooling/sobjects/ApexClass
/// ```
#[allow(async_fn_in_trait)] // Intentional: trait is used internally, Send bound not needed
pub trait RestOperation<A: Authenticator> {
    /// Returns the shared session state containing the HTTP client, token manager,
    /// and configuration.
    fn session(&self) -> &Arc<Session<A>>;

    /// Returns the path prefix for this handler.
    ///
    /// - `""` (empty string) for the standard REST API
    /// - `"tooling"` for the Tooling API
    fn path_prefix(&self) -> &'static str;

    /// Resolves a relative API path by prepending the handler's path prefix.
    ///
    /// If `path_prefix()` is empty, the path is returned as-is. Otherwise the
    /// prefix is prepended with a `/` separator.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // REST (prefix = ""):  "sobjects/Account" → "sobjects/Account"
    /// // Tooling (prefix = "tooling"):  "sobjects/Account" → "tooling/sobjects/Account"
    /// ```
    fn resolve_api_path<'a>(&self, relative_path: &'a str) -> Cow<'a, str> {
        let prefix = self.path_prefix();
        if prefix.is_empty() {
            return Cow::Borrowed(relative_path);
        }

        // ⚡ Bolt: Avoid intermediate string allocation in `format!` macro by pre-allocating
        // the exact capacity needed and writing directly to the buffer.
        let mut out = String::with_capacity(prefix.len() + relative_path.len() + 1);
        out.push_str(prefix);
        out.push('/');
        out.push_str(relative_path);
        Cow::Owned(out)
    }

    // ── CRUD Operations ──────────────────────────────────────────────

    /// Creates a new record in Salesforce.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject type (e.g., `"Account"`, `"Contact"`)
    /// * `data` - JSON object containing field values to set
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The SObject name is invalid
    /// - Authentication fails
    /// - Required fields are missing
    /// - The HTTP request fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use serde_json::json;
    ///
    /// let response = handler.create("Account", &json!({
    ///     "Name": "Acme Corporation"
    /// })).await?;
    /// println!("Created: {}", response.id.unwrap());
    /// ```
    async fn create(&self, sobject: &str, data: &serde_json::Value) -> Result<CreateResponse> {
        validate_sobject_name(sobject)?;
        let relative = crate::api::path_utils::format_sobject_path(sobject, None);
        let api_path = self.resolve_api_path(&relative);
        let url = self.session().resolve_url(&api_path).await?;

        let request = self
            .session()
            .post(&url)
            .json(data)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.session()
            .send_request_and_decode(request, "Create request failed")
            .await
    }

    /// Retrieves a record by its Salesforce ID.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject type
    /// * `id` - The Salesforce ID of the record to retrieve
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The SObject name is invalid
    /// - Authentication fails
    /// - The record does not exist (404)
    /// - The HTTP request fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let id = SalesforceId::new("001xx000003DHP0AAO")?;
    /// let account = handler.get("Account", &id).await?;
    /// println!("Name: {}", account["Name"]);
    /// ```
    async fn get(&self, sobject: &str, id: &SalesforceId) -> Result<serde_json::Value> {
        validate_sobject_name(sobject)?;
        let relative = crate::api::path_utils::format_sobject_path(sobject, Some(id.as_str()));
        let api_path = self.resolve_api_path(&relative);
        let url = self.session().resolve_url(&api_path).await?;

        let request = self
            .session()
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.session()
            .send_request_and_decode(request, "Get request failed")
            .await
    }

    /// Updates an existing record.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject type
    /// * `id` - The Salesforce ID of the record to update
    /// * `data` - JSON object containing field values to update
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The SObject name is invalid
    /// - Authentication fails
    /// - The record does not exist (404)
    /// - The HTTP request fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use serde_json::json;
    ///
    /// let id = SalesforceId::new("001xx000003DHP0AAO")?;
    /// handler.update("Account", &id, &json!({"Phone": "555-0100"})).await?;
    /// ```
    async fn update(
        &self,
        sobject: &str,
        id: &SalesforceId,
        data: &serde_json::Value,
    ) -> Result<UpdateResponse> {
        validate_sobject_name(sobject)?;
        let relative = crate::api::path_utils::format_sobject_path(sobject, Some(id.as_str()));
        let api_path = self.resolve_api_path(&relative);
        let url = self.session().resolve_url(&api_path).await?;

        let request = self
            .session()
            .patch(&url)
            .json(data)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.session()
            .execute_and_check_success(request, "Update request failed")
            .await?;
        Ok(UpdateResponse::success())
    }

    /// Deletes a record.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject type
    /// * `id` - The Salesforce ID of the record to delete
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The SObject name is invalid
    /// - Authentication fails
    /// - The record does not exist (404)
    /// - The HTTP request fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let id = SalesforceId::new("001xx000003DHP0AAO")?;
    /// handler.delete("Account", &id).await?;
    /// ```
    async fn delete(&self, sobject: &str, id: &SalesforceId) -> Result<DeleteResponse> {
        validate_sobject_name(sobject)?;
        let relative = crate::api::path_utils::format_sobject_path(sobject, Some(id.as_str()));
        let api_path = self.resolve_api_path(&relative);
        let url = self.session().resolve_url(&api_path).await?;

        let request = self
            .session()
            .delete(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.session()
            .execute_and_check_success(request, "Delete request failed")
            .await?;
        Ok(DeleteResponse::success())
    }

    /// Upserts a record using an external ID field.
    ///
    /// If a record with the given external ID exists, it will be updated.
    /// Otherwise, a new record will be created.
    ///
    /// Uses the default `Mutation` retry class (non-idempotent, no retry on 503).
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject type
    /// * `external_id_field` - The API name of the external ID field
    /// * `external_id_value` - The value of the external ID
    /// * `data` - JSON object containing field values
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The SObject name or external ID field is invalid
    /// - Authentication fails
    /// - The HTTP request fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use serde_json::json;
    ///
    /// let response = handler.upsert(
    ///     "Account", "ExternalId__c", "ACME-001",
    ///     &json!({"Name": "Acme Corp"})
    /// ).await?;
    /// ```
    async fn upsert(
        &self,
        sobject: &str,
        external_id_field: &str,
        external_id_value: &str,
        data: &serde_json::Value,
    ) -> Result<UpsertResponse> {
        validate_sobject_name(sobject)?;
        validate_external_id_field(external_id_field)?;

        upsert_with_retry_class_impl(
            self.session(),
            self.path_prefix(),
            sobject,
            external_id_field,
            external_id_value,
            data,
            crate::http::RequestRetryClass::Mutation,
        )
        .await
    }

    /// Upserts a record with idempotent retry semantics.
    ///
    /// This method is intended for writes that are safe to retry when transient
    /// infrastructure errors occur (e.g., 503). It routes through the HTTP
    /// executor's `IdempotentMutation` retry class, enabling automatic retry
    /// on transient failures.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject type
    /// * `external_id_field` - The API name of the external ID field
    /// * `external_id_value` - The value of the external ID
    /// * `data` - JSON object containing field values
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The SObject name or external ID field is invalid
    /// - Authentication fails
    /// - The HTTP request fails after retries are exhausted
    async fn upsert_idempotent(
        &self,
        sobject: &str,
        external_id_field: &str,
        external_id_value: &str,
        data: &serde_json::Value,
    ) -> Result<UpsertResponse> {
        validate_sobject_name(sobject)?;
        validate_external_id_field(external_id_field)?;

        upsert_with_retry_class_impl(
            self.session(),
            self.path_prefix(),
            sobject,
            external_id_field,
            external_id_value,
            data,
            crate::http::RequestRetryClass::IdempotentMutation,
        )
        .await
    }

    // ── Query Operations ─────────────────────────────────────────────

    /// Executes a SOQL query and returns the first page of results.
    ///
    /// Use [`query_more()`](Self::query_more) to fetch subsequent pages when
    /// the result set is paginated.
    ///
    /// # Security Warning
    ///
    /// This method accepts a raw SOQL string. **Do not construct queries using
    /// `format!` with untrusted input**, as this leads to SOQL injection
    /// vulnerabilities.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The query is malformed
    /// - Authentication fails
    /// - The HTTP request fails
    /// - Deserialization fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::types::DynamicSObject;
    ///
    /// let result = handler.query::<DynamicSObject>(
    ///     "SELECT Id, Name FROM Account LIMIT 10"
    /// ).await?;
    /// println!("Total: {}", result.total_size);
    /// ```
    async fn query<T>(&self, soql: &str) -> Result<QueryResult<T>>
    where
        T: DeserializeOwned,
    {
        validate_query_input_len("SOQL query", soql)?;

        let api_path = self.resolve_api_path("query");
        let url = self.session().resolve_url(&api_path).await?;

        let request = self
            .session()
            .get(&url)
            .query(&[("q", soql)])
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.session()
            .send_request_and_decode(request, "SOQL query failed")
            .await
    }

    /// Fetches the next page of query results using a `nextRecordsUrl`.
    ///
    /// When a query returns `done: false`, use the `nextRecordsUrl` from the
    /// previous result to fetch the next page.
    ///
    /// # Security
    ///
    /// Absolute URLs are validated against the instance origin to prevent
    /// token leakage to third-party domains.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The locator URL is invalid
    /// - The URL origin does not match the instance (security check)
    /// - Authentication fails
    /// - The HTTP request fails
    /// - Deserialization fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut result = handler.query::<serde_json::Value>("SELECT Id FROM Account").await?;
    /// while !result.is_done() {
    ///     if let Some(next_url) = result.next_records_url.as_ref() {
    ///         result = handler.query_more(next_url).await?;
    ///     }
    /// }
    /// ```
    async fn query_more<T>(&self, next_records_url: &str) -> Result<QueryResult<T>>
    where
        T: DeserializeOwned,
    {
        validate_query_input_len("next_records_url", next_records_url)?;

        let instance_url = self.session().instance_url().await?;
        let url = resolve_next_records_url(&instance_url, next_records_url)?;

        let request = self
            .session()
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.session()
            .send_request_and_decode(request, "Query pagination failed")
            .await
    }

    /// Creates a stream of query results for the given SOQL.
    ///
    /// This method simplifies paginated queries by returning a stream that automatically
    /// fetches subsequent pages of results as needed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let stream = client.rest().query_stream::<Account>("SELECT Id FROM Account");
    /// ```
    fn query_stream<T>(
        &self,
        soql: impl Into<String>,
    ) -> crate::api::query_stream::QueryStream<T, A, Self>
    where
        T: DeserializeOwned + Unpin,
        Self: Sized + Clone,
    {
        crate::api::query_stream::QueryStream::new(self.clone(), soql)
    }

    // ── Describe Operations ──────────────────────────────────────────

    /// Retrieves global describe information.
    ///
    /// Returns metadata for all available SObjects in the organization.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let global = handler.describe_global().await?;
    /// for sobject in &global.sobjects {
    ///     println!("{}: {}", sobject.name, sobject.label);
    /// }
    /// ```
    async fn describe_global(&self) -> Result<GlobalDescribe> {
        let api_path = self.resolve_api_path("sobjects");
        let url = self.session().resolve_url(&api_path).await?;

        let request = self
            .session()
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.session()
            .send_request_and_decode(request, "Global describe request failed")
            .await
    }

    /// Retrieves detailed metadata for a specific SObject.
    ///
    /// Returns comprehensive information including all fields, relationships,
    /// record types, and other metadata for the specified object.
    ///
    /// # Arguments
    ///
    /// * `sobject_type` - The API name of the SObject (e.g., `"Account"`, `"Contact"`)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The SObject does not exist
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let describe = handler.describe("Account").await?;
    /// println!("Object: {} ({})", describe.name, describe.label);
    /// for field in &describe.fields {
    ///     println!("  {} - {:?}", field.name, field.type_);
    /// }
    /// ```
    async fn describe(&self, sobject_type: &str) -> Result<SObjectDescribe> {
        validate_sobject_name(sobject_type)?;
        let mut relative = crate::api::path_utils::format_sobject_path(sobject_type, None);
        relative.push_str("/describe");
        let api_path = self.resolve_api_path(&relative);
        let url = self.session().resolve_url(&api_path).await?;

        let request = self
            .session()
            .get(&url)
            .build()
            .map_err(crate::error::HttpError::from)?;

        self.session()
            .send_request_and_decode(request, "Describe request failed")
            .await
    }
}

// ── Private helper methods ───────────────────────────────────────────

fn validate_query_input_len(name: &str, value: &str) -> Result<()> {
    if value.len() > MAX_QUERY_INPUT_BYTES {
        return Err(ForceError::InvalidInput(format!(
            "{name} exceeds maximum allowed length of 100,000 bytes"
        )));
    }

    Ok(())
}

/// Internal helper shared by [`RestOperation::upsert`] and
/// [`RestOperation::upsert_idempotent`].
///
/// Extracted as a standalone async function rather than a default trait method
/// to keep the public trait surface clean while avoiding code duplication.
async fn upsert_with_retry_class_impl<A: Authenticator>(
    session: &Arc<Session<A>>,
    api_path_prefix: &str,
    sobject: &str,
    external_id_field: &str,
    external_id_value: &str,
    data: &serde_json::Value,
    retry_class: crate::http::RequestRetryClass,
) -> Result<UpsertResponse> {
    use std::fmt::Write;

    validate_sobject_name(sobject)?;
    validate_external_id_field(external_id_field)?;

    let encoded_value = utf8_percent_encode(external_id_value, UPSERT_ENCODE_SET);

    // ⚡ Bolt: Avoid intermediate `format!` allocation by writing directly to a pre-allocated buffer.
    let mut api_path = String::with_capacity(
        api_path_prefix.len()
            + sobject.len()
            + external_id_field.len()
            + external_id_value.len()
            + 24,
    );
    if api_path_prefix.is_empty() {
        write!(
            api_path,
            "sobjects/{}/{}/{}",
            sobject, external_id_field, encoded_value
        )
        .unwrap_or_else(|_| unreachable!("writing to String is infallible"));
    } else {
        write!(
            api_path,
            "{}/sobjects/{}/{}/{}",
            api_path_prefix, sobject, external_id_field, encoded_value
        )
        .unwrap_or_else(|_| unreachable!("writing to String is infallible"));
    }
    let url = session.resolve_url(&api_path).await?;

    let request = session
        .patch(&url)
        .json(data)
        .build()
        .map_err(crate::error::HttpError::from)?;

    let response = session
        .execute_request_with_retry_class(request, retry_class)
        .await?;

    let status = response.status();

    if status.as_u16() == 204 {
        // 204 No Content means an existing record was updated
        // But the response does not include the record ID
        return Err(ForceError::NotImplemented(
            "Upsert update (204) response does not include record ID - use query to retrieve"
                .to_string(),
        ));
    }

    if status.is_success() {
        // Success codes (201 Created, 200 OK) - parse as upsert response
        let bytes = crate::http::error::read_capped_body_bytes(response, 100 * 1024 * 1024).await?;
        return serde_json::from_slice::<UpsertResponse>(&bytes)
            .map_err(|e| crate::error::SerializationError::from(e).into());
    }

    Err(crate::http::response_to_force_error(response, "Upsert request failed").await)
}

/// Resolves and validates the `nextRecordsUrl` for query pagination.
///
/// Ensures that absolute URLs match the instance origin to prevent
/// token leakage to third-party domains.
///
/// # Security
///
/// - Relative URLs (e.g., `/services/data/v60.0/query/01g-2000`) are
///   prefixed with the instance URL.
/// - Absolute URLs are validated: scheme, host, port must match the instance,
///   and no embedded credentials are allowed.
///
/// # Errors
///
/// Returns [`ForceError::InvalidInput`] if:
/// - The URL cannot be parsed
/// - The origin does not match the instance
/// - Credentials are embedded in the URL
pub fn resolve_next_records_url(instance_url: &str, next_records_url: &str) -> Result<String> {
    let instance_parsed = url::Url::parse(instance_url)
        .map_err(|e| ForceError::InvalidInput(format!("Invalid instance URL in token: {}", e)))?;

    // Combine safely using `url::Url::join` which handles relative vs absolute correctly
    // and prevents `@evil.com` from changing the host.
    let next_parsed = instance_parsed
        .join(next_records_url)
        .map_err(|e| ForceError::InvalidInput(format!("Invalid nextRecordsUrl: {}", e)))?;

    // Security check: the resolved absolute URL must match the instance host
    validate_url_origin_match(&instance_parsed, &next_parsed)?;

    // ⚡ Bolt: Avoid unnecessary heap allocation and copy by using `.into()` to consume the `Url` and yield its internal string buffer instead of `.to_string()`.
    Ok(next_parsed.into())
}

/// Helper function to validate that the origin and credentials of an absolute URL match the instance.
fn validate_url_origin_match(instance: &url::Url, next: &url::Url) -> Result<()> {
    let scheme_mismatch = next.scheme() != instance.scheme();
    let host_mismatch = next.host_str() != instance.host_str();
    let port_mismatch = next.port_or_known_default() != instance.port_or_known_default();
    let has_credentials = !next.username().is_empty() || next.password().is_some();

    if scheme_mismatch || host_mismatch || port_mismatch || has_credentials {
        return Err(ForceError::InvalidInput(format!(
            "Security Error: nextRecordsUrl origin ({:?}://{:?}:{:?}) does not match instance origin ({:?}://{:?}:{:?})",
            next.scheme(),
            next.host_str(),
            next.port_or_known_default(),
            instance.scheme(),
            instance.host_str(),
            instance.port_or_known_default()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;

    // ── resolve_next_records_url unit tests ──────────────────────────

    #[test]
    fn test_resolve_relative_url() {
        let result = resolve_next_records_url(
            "https://na1.salesforce.com",
            "/services/data/v60.0/query/01g-2000",
        )
        .must();
        assert_eq!(
            result,
            "https://na1.salesforce.com/services/data/v60.0/query/01g-2000"
        );
    }

    #[test]
    fn test_resolve_absolute_url_same_origin() {
        let result = resolve_next_records_url(
            "https://na1.salesforce.com",
            "https://na1.salesforce.com/services/data/v60.0/query/01g-2000",
        )
        .must();
        assert_eq!(
            result,
            "https://na1.salesforce.com/services/data/v60.0/query/01g-2000"
        );
    }

    #[test]
    fn test_resolve_absolute_url_different_host_rejected() {
        let result = resolve_next_records_url(
            "https://na1.salesforce.com",
            "https://attacker.com/services/data/v60.0/query/leak",
        );
        let Err(err) = result else {
            panic!("Expected Err");
        };
        assert!(err.to_string().contains("Security Error"));
    }

    #[test]
    fn test_resolve_absolute_url_scheme_mismatch_rejected() {
        let result = resolve_next_records_url(
            "https://na1.salesforce.com",
            "http://na1.salesforce.com/services/data/v60.0/query/01g",
        );
        let Err(err) = result else {
            panic!("Expected Err");
        };
        assert!(err.to_string().contains("Security Error"));
    }

    #[test]
    fn test_resolve_absolute_url_port_mismatch_rejected() {
        let result = resolve_next_records_url(
            "https://na1.salesforce.com",
            "https://na1.salesforce.com:9999/services/data/v60.0/query/01g",
        );
        let Err(err) = result else {
            panic!("Expected Err");
        };
        assert!(err.to_string().contains("Security Error"));
    }

    #[test]
    fn test_resolve_absolute_url_with_username_rejected() {
        let result = resolve_next_records_url(
            "https://na1.salesforce.com",
            "https://attacker@na1.salesforce.com/services/data/v60.0/query/01g",
        );
        let Err(err) = result else {
            panic!("Expected Err");
        };
        assert!(err.to_string().contains("Security Error"));
    }

    #[test]
    fn test_resolve_absolute_url_with_credentials_rejected() {
        let result = resolve_next_records_url(
            "https://na1.salesforce.com",
            "https://user:pass@na1.salesforce.com/services/data/v60.0/query/01g",
        );
        let Err(err) = result else {
            panic!("Expected Err");
        };
        assert!(err.to_string().contains("Security Error"));
    }

    // ── resolve_api_path unit tests ─────────────────────────────────

    /// Minimal test implementor with no prefix (REST API).
    #[derive(Clone)]
    struct TestRestOp;

    impl RestOperation<crate::test_utils::mock_auth::MockAuthenticator> for TestRestOp {
        fn session(&self) -> &Arc<Session<crate::test_utils::mock_auth::MockAuthenticator>> {
            panic!("validation should fail before session access")
        }
        fn path_prefix(&self) -> &'static str {
            ""
        }
    }

    /// Minimal test implementor with "tooling" prefix.
    #[derive(Clone)]
    struct TestToolingOp;

    impl RestOperation<crate::test_utils::mock_auth::MockAuthenticator> for TestToolingOp {
        fn session(&self) -> &Arc<Session<crate::test_utils::mock_auth::MockAuthenticator>> {
            unimplemented!("not needed for path tests")
        }
        fn path_prefix(&self) -> &'static str {
            "tooling"
        }
    }

    #[test]
    fn test_resolve_api_path_no_prefix() {
        let op = TestRestOp;
        assert_eq!(op.resolve_api_path("sobjects/Account"), "sobjects/Account");
        assert_eq!(op.resolve_api_path("query"), "query");
        assert_eq!(op.resolve_api_path("sobjects"), "sobjects");
    }

    #[test]
    fn test_resolve_api_path_with_prefix() {
        let op = TestToolingOp;
        assert_eq!(
            op.resolve_api_path("sobjects/Account"),
            "tooling/sobjects/Account"
        );
        assert_eq!(op.resolve_api_path("query"), "tooling/query");
        assert_eq!(op.resolve_api_path("sobjects"), "tooling/sobjects");
    }

    fn assert_invalid_input_contains<T>(result: Result<T>, expected: &str) {
        let Err(ForceError::InvalidInput(message)) = result else {
            panic!("expected InvalidInput containing {expected:?}");
        };
        assert!(
            message.contains(expected),
            "expected {message:?} to contain {expected:?}"
        );
    }

    #[tokio::test]
    async fn test_validation_query_rejects_oversized_soql_before_session() {
        let op = TestRestOp;
        let soql = "A".repeat(MAX_QUERY_INPUT_BYTES + 1);
        let result = op.query::<serde_json::Value>(&soql).await;

        assert_invalid_input_contains(result, "100,000 bytes");
    }

    #[tokio::test]
    async fn test_validation_query_more_rejects_oversized_url_before_session() {
        let op = TestRestOp;
        let next_records_url = "A".repeat(MAX_QUERY_INPUT_BYTES + 1);
        let result = op.query_more::<serde_json::Value>(&next_records_url).await;

        assert_invalid_input_contains(result, "100,000 bytes");
    }

    #[tokio::test]
    async fn test_validation_describe_rejects_invalid_sobject_before_session() {
        let op = TestRestOp;
        let result = op.describe("Account;DROP").await;

        assert_invalid_input_contains(result, "SObject name contains invalid characters");
    }

    #[tokio::test]
    async fn test_validation_create_rejects_invalid_sobject_before_session() {
        let op = TestRestOp;
        let result = op.create("Account;DROP", &serde_json::json!({})).await;
        assert_invalid_input_contains(result, "SObject name contains invalid characters");
    }

    #[tokio::test]
    async fn test_validation_update_rejects_invalid_names_before_session() {
        let op = TestRestOp;
        let id = crate::types::SalesforceId::new("001000000000001AAA").must();
        let result = op.update("Account;DROP", &id, &serde_json::json!({})).await;
        assert_invalid_input_contains(result, "SObject name contains invalid characters");
    }

    #[tokio::test]
    async fn test_validation_delete_rejects_invalid_names_before_session() {
        let op = TestRestOp;
        let id = crate::types::SalesforceId::new("001000000000001AAA").must();
        let result = op.delete("Account;DROP", &id).await;
        assert_invalid_input_contains(result, "SObject name contains invalid characters");
    }

    #[tokio::test]
    async fn test_validation_get_rejects_invalid_names_before_session() {
        let op = TestRestOp;
        let id = crate::types::SalesforceId::new("001000000000001AAA").must();
        let result = op.get("Account;DROP", &id).await;
        assert_invalid_input_contains(result, "SObject name contains invalid characters");
    }

    #[tokio::test]
    async fn test_validation_upsert_rejects_invalid_names_before_session() {
        let op = TestRestOp;

        let result = op
            .upsert("Account;DROP", "ExtId", "123", &serde_json::json!({}))
            .await;
        assert_invalid_input_contains(result, "SObject name contains invalid characters");

        let result = op
            .upsert("Account", "ExtId;DROP", "123", &serde_json::json!({}))
            .await;
        assert_invalid_input_contains(result, "External ID field name contains invalid characters");
    }

    #[tokio::test]
    async fn test_upsert_returns_not_implemented_on_204() {
        use crate::client::builder;
        use serde_json::json;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/ExternalId__c/123",
            ))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();

        let result = client
            .rest()
            .upsert("Account", "ExternalId__c", "123", &json!({"Name": "Acme"}))
            .await;

        match result {
            Err(crate::error::ForceError::NotImplemented(msg)) => {
                assert_eq!(
                    msg,
                    "Upsert update (204) response does not include record ID - use query to retrieve"
                );
            }
            _ => panic!("Expected NotImplemented error for 204 response"),
        }
    }

    #[tokio::test]
    async fn test_upsert_success_other_status() {
        use crate::client::builder;
        use serde_json::json;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        // Testing the `_ if response.status().is_success()` match arm directly
        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-002",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "001xx000003DHP0AAO",
                "success": true,
                "created": false,
                "errors": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let response = rest
            .upsert(
                "Account",
                "ExternalId__c",
                "ACME-002",
                &json!({"Name": "Acme Corp 2"}),
            )
            .await
            .must();

        assert!(response.is_success());
        assert!(!response.is_created());
        assert_eq!(response.id.as_str(), "001xx000003DHP0AAO");
    }

    #[tokio::test]
    async fn test_upsert_failure() {
        use crate::client::builder;
        use serde_json::json;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-003",
            ))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
                "message": "Bad Request",
                "errorCode": "BAD_REQUEST"
            }])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let result = rest
            .upsert(
                "Account",
                "ExternalId__c",
                "ACME-003",
                &json!({"Name": "Acme Corp 3"}),
            )
            .await;

        let Err(err) = result else {
            panic!("Expected Err");
        };
        assert!(err.to_string().contains("Bad Request"));
    }

    #[tokio::test]
    async fn test_get_success_mock() {
        use crate::client::builder;
        use crate::types::SalesforceId;
        use serde_json::json;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/001xx000003DHP0AAO",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Id": "001xx000003DHP0AAO",
                "Name": "Test Account"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let id = SalesforceId::new("001xx000003DHP0AAO").must();
        let result = rest.get("Account", &id).await.must();

        assert_eq!(result["Name"], "Test Account");
    }

    #[tokio::test]
    async fn test_create_success_mock() {
        use crate::client::builder;
        use serde_json::json;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/sobjects/Account"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "001xx000003DHP0AAO",
                "success": true,
                "errors": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let response = rest
            .create("Account", &json!({"Name": "New Account"}))
            .await
            .must();

        assert!(response.is_success());
        assert_eq!(response.id.must().as_str(), "001xx000003DHP0AAO");
    }

    #[tokio::test]
    async fn test_update_success_mock() {
        use crate::client::builder;
        use crate::types::SalesforceId;
        use serde_json::json;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/001xx000003DHP0AAO",
            ))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let id = SalesforceId::new("001xx000003DHP0AAO").must();
        let response = rest
            .update("Account", &id, &json!({"Name": "Updated Account"}))
            .await
            .must();

        assert!(response.is_success());
    }

    #[tokio::test]
    async fn test_delete_success_mock() {
        use crate::client::builder;
        use crate::types::SalesforceId;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("DELETE"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/001xx000003DHP0AAO",
            ))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let id = SalesforceId::new("001xx000003DHP0AAO").must();
        let response = rest.delete("Account", &id).await.must();

        assert!(response.is_success());
    }

    #[tokio::test]
    async fn test_upsert_idempotent_success_mock() {
        use crate::client::builder;
        use serde_json::json;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-005",
            ))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "001xx000003DHP0AAO",
                "success": true,
                "created": true,
                "errors": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let response = rest
            .upsert_idempotent(
                "Account",
                "ExternalId__c",
                "ACME-005",
                &json!({"Name": "Idempotent Account"}),
            )
            .await
            .must();

        assert!(response.is_success());
        assert!(response.is_created());
        assert_eq!(response.id.as_str(), "001xx000003DHP0AAO");
    }

    #[tokio::test]
    async fn test_describe_global_success_mock() {
        use crate::client::builder;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let global_describe_json: serde_json::Value = serde_json::from_str(
            r#"{
            "encoding": "UTF-8",
            "maxBatchSize": 200,
            "sobjects": [
                {
                    "name": "Account",
                    "label": "Account",
                    "custom": false,
                    "keyPrefix": "001",
                    "urls": {},
                    "activateable": false,
                    "createable": true,
                    "customSetting": false,
                    "deletable": true,
                    "deprecatedAndHidden": false,
                    "feedEnabled": false,
                    "hasSubtypes": false,
                    "isSubtype": false,
                    "labelPlural": "Accounts",
                    "layoutable": true,
                    "mergeable": true,
                    "mruEnabled": true,
                    "queryable": true,
                    "replicateable": true,
                    "retrieveable": true,
                    "searchable": true,
                    "triggerable": true,
                    "undeletable": true,
                    "updateable": true
                }
            ]
        }"#,
        )
        .must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(global_describe_json))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let response = rest.describe_global().await.must();

        assert_eq!(response.sobjects.len(), 1);
        assert_eq!(response.sobjects[0].name, "Account");
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn test_describe_success_mock() {
        use crate::client::builder;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let describe_json: serde_json::Value = serde_json::from_str(
            r#"{
            "name": "Account",
            "label": "Account",
            "custom": false,
            "keyPrefix": "001",
            "activateable": false,
            "createable": true,
            "customSetting": false,
            "deletable": true,
            "deprecatedAndHidden": false,
            "feedEnabled": false,
            "hasSubtypes": false,
            "isSubtype": false,
            "labelPlural": "Accounts",
            "layoutable": true,
            "mergeable": true,
            "mruEnabled": true,
            "queryable": true,
            "replicateable": true,
            "retrieveable": true,
            "searchable": true,
            "triggerable": true,
            "undeletable": true,
            "updateable": true,
            "childRelationships": [],
            "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Id",
                    "type": "id",
                    "label": "Record ID",
                    "length": 18,
                    "nillable": false,
                    "custom": false,
                    "calculated": false,
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 18,
                    "caseSensitive": false,
                    "createable": false,
                    "defaultedOnCreate": true,
                    "deprecatedAndHidden": false,
                    "digits": 0,
                    "filterable": true,
                    "groupable": true,
                    "idLookup": true,
                    "nameField": false,
                    "namePointing": false,
                    "permissionable": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "sortable": true,
                    "unique": false,
                    "updateable": false,
                    "cascadeDelete": false,
                    "dependentPicklist": false,
                    "deprecatedAndHidden": false,
                    "displayLocationInDecimal": false,
                    "encrypted": false,
                    "externalId": false,
                    "highScaleNumber": false,
                    "htmlFormatted": false,
                    "polymorphicForeignKey": false,
                    "searchPrefilterable": false,
                    "writeRequiresMasterRead": false,
                    "precision": 0,
                    "queryByDistance": false,
                    "restrictedDelete": false,
                    "sortable": true,
                    "unique": false,
                    "updateable": false,
                    "referenceTo": [],
                    "relationshipName": null,
                    "relationshipOrder": null,
                    "referenceTargetField": null,
                    "soapType": "tns:ID"
                }
            ],
            "urls": {}
        }"#,
        )
        .must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let response = rest.describe("Account").await.must();

        assert_eq!(response.name, "Account");
        assert_eq!(response.fields.len(), 1);
        assert_eq!(response.fields[0].name, "Id");
    }

    #[tokio::test]
    async fn test_query_success_mock() {
        use crate::client::builder;
        use serde_json::json;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 1,
                "done": true,
                "records": [{"Id": "001xx000003DHP0AAO", "Name": "Test Account"}]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let result = rest
            .query::<serde_json::Value>("SELECT Id, Name FROM Account")
            .await
            .must();

        assert_eq!(result.total_size, 1);
        assert!(result.done);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0]["Name"], "Test Account");
    }

    #[tokio::test]
    async fn test_query_more_success_mock() {
        use crate::client::builder;
        use serde_json::json;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let auth =
            crate::test_utils::mock_auth::MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query/01g"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 2,
                "done": true,
                "records": [{"Id": "001xx000003DHP0AAO", "Name": "Test Account 2"}]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let result = rest
            .query_more::<serde_json::Value>("/services/data/v60.0/query/01g")
            .await
            .must();

        assert_eq!(result.total_size, 2);
        assert!(result.done);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0]["Name"], "Test Account 2");
    }

    #[test]
    fn test_query_more_security_check_scheme_mismatch() {
        let result = resolve_next_records_url(
            "https://na1.salesforce.com",
            "http://na1.salesforce.com/services/data/v60.0/query/01g",
        );
        let Err(err) = result else {
            panic!("Expected Err");
        };
        assert!(err.to_string().contains("Security Error"));
    }

    #[test]
    fn test_query_more_security_check_port_mismatch() {
        let result = resolve_next_records_url(
            "https://na1.salesforce.com",
            "https://na1.salesforce.com:8080/services/data/v60.0/query/01g",
        );
        let Err(err) = result else {
            panic!("Expected Err");
        };
        assert!(err.to_string().contains("Security Error"));
    }

    #[test]
    fn test_query_more_security_check_username_mismatch() {
        let result = resolve_next_records_url(
            "https://na1.salesforce.com",
            "https://user@na1.salesforce.com/services/data/v60.0/query/01g",
        );
        let Err(err) = result else {
            panic!("Expected Err");
        };
        assert!(err.to_string().contains("Security Error"));
    }

    #[test]
    fn test_query_more_security_check_password_mismatch() {
        let result = resolve_next_records_url(
            "https://na1.salesforce.com",
            "https://:password@na1.salesforce.com/services/data/v60.0/query/01g",
        );
        let Err(err) = result else {
            panic!("Expected Err");
        };
        assert!(err.to_string().contains("Security Error"));
    }
}
