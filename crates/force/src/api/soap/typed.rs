//! Serde-typed convenience layer over the untyped SOAP Partner API.
//!
//! The Partner WSDL is untyped: every field crosses the wire as a string and the
//! base [`SObject`] models a record as a type name plus a bag of string fields.
//! This module bridges caller structs to and from that bag with `serde_json`, so
//! callers can work with their own `#[derive(Serialize, Deserialize)]` types
//! instead of the generic field map. It is a thin adapter — every method
//! delegates to an existing untyped method for the actual XML and transport.
//!
//! # Stringly-typed fields
//!
//! Because the Partner API returns **all** field values as strings, deserialized
//! target types should model fields as `String` / `Option<String>`, or supply a
//! `#[serde(deserialize_with = "…")]` that parses a string into another type.
//! Numbers and booleans are serialized to their plain string forms on the way
//! out (`100`, `true`), matching what the wire expects.
//!
//! # Null handling
//!
//! A JSON `null` in a serialized record maps to a `fieldsToNull` entry (the
//! Partner API's explicit-null mechanism) rather than an empty element, matching
//! Salesforce semantics for `update`/`upsert`.

use super::{SObject, SaveResult, SoapHandler, UpsertResult, crud, parse};
use crate::error::{ForceError, Result, SerializationError};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Converts a serializable record into an [`SObject`] of the given type.
///
/// Scalar JSON values become string fields; JSON `null` becomes a `fieldsToNull`
/// entry. Nested arrays/objects are rejected: the Partner sObject field bag is
/// flat and cannot represent them.
fn sobject_from_serializable<T: Serialize>(sobject_type: &str, record: &T) -> Result<SObject> {
    let value = serde_json::to_value(record).map_err(SerializationError::from)?;
    let serde_json::Value::Object(map) = value else {
        return Err(ForceError::InvalidInput(
            "SOAP typed record must serialize to a JSON object".to_string(),
        ));
    };
    let mut obj = SObject::new(sobject_type);
    for (key, val) in map {
        match val {
            serde_json::Value::Null => obj.fields_to_null.push(key),
            serde_json::Value::String(s) => obj.fields.push((key, Some(s))),
            serde_json::Value::Bool(b) => obj.fields.push((key, Some(b.to_string()))),
            serde_json::Value::Number(n) => obj.fields.push((key, Some(n.to_string()))),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                return Err(ForceError::InvalidInput(format!(
                    "SOAP typed record field `{key}` serializes to a nested \
                     array/object, which the Partner sObject field model does not support"
                )));
            }
        }
    }
    Ok(obj)
}

/// Deserializes an [`SObject`]'s field map into a caller type.
///
/// Non-null fields become JSON strings and nil fields become JSON `null`, then
/// the assembled object is handed to `serde_json`.
fn sobject_into_typed<T: DeserializeOwned>(obj: &SObject) -> Result<T> {
    let mut map = serde_json::Map::with_capacity(obj.fields.len());
    for (name, value) in &obj.fields {
        let json_value = match value {
            Some(s) => serde_json::Value::String(s.clone()),
            None => serde_json::Value::Null,
        };
        map.insert(name.clone(), json_value);
    }
    serde_json::from_value(serde_json::Value::Object(map))
        .map_err(|e| ForceError::from(SerializationError::from(e)))
}

/// Deserializes a slice of records into a vector of caller types, short-circuiting
/// on the first deserialization failure.
fn records_to_typed<T: DeserializeOwned>(records: &[SObject]) -> Result<Vec<T>> {
    records.iter().map(sobject_into_typed).collect()
}

impl<A: crate::auth::Authenticator> SoapHandler<A> {
    /// Runs a SOQL query and deserializes **all** matching records into `T`.
    ///
    /// This auto-follows `queryMore` locators, so the returned vector spans every
    /// page of the result set. For very large result sets that should be consumed
    /// incrementally, drive [`query`](Self::query) / [`query_more`](Self::query_more)
    /// (or [`query_typed_page`](Self::query_typed_page)) yourself instead.
    ///
    /// Because the Partner API returns every field as a string, `T`'s fields
    /// should be `String` / `Option<String>` (or use `#[serde(deserialize_with)]`).
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`] on a transport failure, a
    /// SOAP fault, an XML parse error, or if a record fails to deserialize into `T`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// #[derive(serde::Deserialize)]
    /// struct Account {
    ///     #[serde(rename = "Id")]
    ///     id: String,
    ///     #[serde(rename = "Name")]
    ///     name: String,
    /// }
    ///
    /// let accounts: Vec<Account> = client
    ///     .soap()
    ///     .query_typed("SELECT Id, Name FROM Account")
    ///     .await?;
    /// ```
    pub async fn query_typed<T: DeserializeOwned>(&self, soql: &str) -> Result<Vec<T>> {
        let mut page = self.query(soql).await?;
        let mut out = records_to_typed(&page.records)?;
        while !page.done {
            let Some(locator) = page.query_locator.as_deref() else {
                break;
            };
            page = self.query_more(locator).await?;
            out.extend(records_to_typed::<T>(&page.records)?);
        }
        Ok(out)
    }

    /// Runs a SOQL query and deserializes only the **first page** into `T`.
    ///
    /// Returns the typed records alongside the raw `done` flag and the next-page
    /// `queryLocator`. When `done` is `false`, pass the locator to
    /// [`query_more_typed_page`](Self::query_more_typed_page) to continue.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`] on a transport failure, a
    /// SOAP fault, an XML parse error, or if a record fails to deserialize into `T`.
    pub async fn query_typed_page<T: DeserializeOwned>(
        &self,
        soql: &str,
    ) -> Result<(Vec<T>, bool, Option<String>)> {
        let page = self.query(soql).await?;
        let records = records_to_typed(&page.records)?;
        Ok((records, page.done, page.query_locator))
    }

    /// Fetches the next typed page using a locator from a prior typed page.
    ///
    /// Returns the typed records alongside the `done` flag and the next
    /// `queryLocator`, mirroring [`query_typed_page`](Self::query_typed_page).
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`] on a transport failure, a
    /// SOAP fault, an XML parse error, or if a record fails to deserialize into `T`.
    pub async fn query_more_typed_page<T: DeserializeOwned>(
        &self,
        query_locator: &str,
    ) -> Result<(Vec<T>, bool, Option<String>)> {
        let page = self.query_more(query_locator).await?;
        let records = records_to_typed(&page.records)?;
        Ok((records, page.done, page.query_locator))
    }

    /// Retrieves records by Id and deserializes each slot into `T`.
    ///
    /// The returned vector is positional: entry `i` corresponds to `ids[i]`, and a
    /// not-found Id yields `None` (unlike the untyped [`retrieve`](Self::retrieve),
    /// which drops missing records).
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`] on a transport failure, a
    /// SOAP fault, an XML parse error, or if a record fails to deserialize into `T`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// #[derive(serde::Deserialize)]
    /// struct Account {
    ///     #[serde(rename = "Name")]
    ///     name: String,
    /// }
    ///
    /// let accounts: Vec<Option<Account>> = client
    ///     .soap()
    ///     .retrieve_typed("Account", &["Name"], &["001AAA", "001MISSING"])
    ///     .await?;
    /// ```
    pub async fn retrieve_typed<T, F, I>(
        &self,
        sobject_type: &str,
        fields: &[F],
        ids: &[I],
    ) -> Result<Vec<Option<T>>>
    where
        T: DeserializeOwned,
        F: AsRef<str> + Sync,
        I: AsRef<str> + Sync,
    {
        let body = crud::build_retrieve_body(sobject_type, fields, ids);
        let xml = self.send(&body).await?;
        parse::parse_retrieve_optional(&xml)?
            .iter()
            .map(|slot| slot.as_ref().map(sobject_into_typed).transpose())
            .collect()
    }

    /// Serializes and creates one or more typed records of `sobject_type`.
    ///
    /// The object type is taken as an explicit parameter (rather than a trait on
    /// `T`) so callers need not implement anything on their structs. Each record
    /// is serialized into an [`SObject`] and delegated to
    /// [`create`](Self::create); returns one [`SaveResult`] per input record.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`] on a transport failure, a
    /// SOAP fault, an XML parse error, or if a record cannot be serialized into a
    /// flat field map (for example, it contains a nested array/object field).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// #[derive(serde::Serialize)]
    /// struct NewAccount {
    ///     #[serde(rename = "Name")]
    ///     name: String,
    /// }
    ///
    /// let saves = client
    ///     .soap()
    ///     .create_typed("Account", &[NewAccount { name: "Acme".into() }])
    ///     .await?;
    /// assert!(saves[0].success);
    /// ```
    pub async fn create_typed<T: Serialize + Sync>(
        &self,
        sobject_type: &str,
        records: &[T],
    ) -> Result<Vec<SaveResult>> {
        let objects = records
            .iter()
            .map(|record| sobject_from_serializable(sobject_type, record))
            .collect::<Result<Vec<_>>>()?;
        self.create(&objects).await
    }

    /// Serializes and updates one or more typed records of `sobject_type`.
    ///
    /// Each serialized record must include an `Id` field (use
    /// `#[serde(rename = "Id")]`). Delegates to [`update`](Self::update).
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`] on a transport failure, a
    /// SOAP fault, an XML parse error, or if a record cannot be serialized into a
    /// flat field map.
    pub async fn update_typed<T: Serialize + Sync>(
        &self,
        sobject_type: &str,
        records: &[T],
    ) -> Result<Vec<SaveResult>> {
        let objects = records
            .iter()
            .map(|record| sobject_from_serializable(sobject_type, record))
            .collect::<Result<Vec<_>>>()?;
        self.update(&objects).await
    }

    /// Serializes and upserts one or more typed records of `sobject_type`.
    ///
    /// Records are keyed on `external_id_field`. Delegates to
    /// [`upsert`](Self::upsert); returns one [`UpsertResult`] per input record.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`] on a transport failure, a
    /// SOAP fault, an XML parse error, or if a record cannot be serialized into a
    /// flat field map.
    pub async fn upsert_typed<T: Serialize + Sync>(
        &self,
        sobject_type: &str,
        external_id_field: &str,
        records: &[T],
    ) -> Result<Vec<UpsertResult>> {
        let objects = records
            .iter()
            .map(|record| sobject_from_serializable(sobject_type, record))
            .collect::<Result<Vec<_>>>()?;
        self.upsert(external_id_field, &objects).await
    }
}
