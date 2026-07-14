//! CRUD calls for the SOAP Partner API: `create`, `update`, `upsert`,
//! `delete`, and `retrieve`.

use super::{DeleteResult, SObject, SaveResult, SoapHandler, UpsertResult, envelope, parse};
use crate::error::Result;

impl<A: crate::auth::Authenticator> SoapHandler<A> {
    /// Creates one or more records (up to 200 per call).
    ///
    /// Returns one [`SaveResult`] per input record, in order. A record that
    /// fails to save is reported with `success = false` and a populated
    /// `errors` list — it is not an error for this call.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn create(&self, records: &[SObject]) -> Result<Vec<SaveResult>> {
        let mut body = String::with_capacity(records.len().saturating_mul(128) + 32);
        body.push_str("<urn:create>");
        for record in records {
            envelope::serialize_sobject(&mut body, record);
        }
        body.push_str("</urn:create>");
        let xml = self.send(&body).await?;
        parse::parse_save_results(&xml)
    }

    /// Updates one or more records (up to 200 per call).
    ///
    /// Each record must include its `Id` field. Returns one [`SaveResult`] per
    /// input record, in order.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn update(&self, records: &[SObject]) -> Result<Vec<SaveResult>> {
        let mut body = String::with_capacity(records.len().saturating_mul(128) + 32);
        body.push_str("<urn:update>");
        for record in records {
            envelope::serialize_sobject(&mut body, record);
        }
        body.push_str("</urn:update>");
        let xml = self.send(&body).await?;
        parse::parse_save_results(&xml)
    }

    /// Creates or updates records keyed on an external ID field (up to 200 per call).
    ///
    /// Returns one [`UpsertResult`] per input record, in order; each reports
    /// whether the record was `created`.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn upsert(
        &self,
        external_id_field: &str,
        records: &[SObject],
    ) -> Result<Vec<UpsertResult>> {
        let mut body = String::with_capacity(records.len().saturating_mul(128) + 64);
        body.push_str("<urn:upsert><urn:externalIDFieldName>");
        body.push_str(envelope::escape_text(external_id_field).as_ref());
        body.push_str("</urn:externalIDFieldName>");
        for record in records {
            envelope::serialize_sobject(&mut body, record);
        }
        body.push_str("</urn:upsert>");
        let xml = self.send(&body).await?;
        parse::parse_upsert_results(&xml)
    }

    /// Deletes records by Id (up to 200 per call).
    ///
    /// Returns one [`DeleteResult`] per input Id, in order.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn delete<S: AsRef<str> + Sync>(&self, ids: &[S]) -> Result<Vec<DeleteResult>> {
        let mut body = String::with_capacity(ids.len().saturating_mul(48) + 32);
        body.push_str("<urn:delete>");
        for id in ids {
            body.push_str("<urn:ids>");
            body.push_str(envelope::escape_text(id.as_ref()).as_ref());
            body.push_str("</urn:ids>");
        }
        body.push_str("</urn:delete>");
        let xml = self.send(&body).await?;
        parse::parse_delete_results(&xml)
    }

    /// Retrieves records of a single type by Id, returning the requested fields.
    ///
    /// Not-found Ids are omitted from the returned vector.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn retrieve<F: AsRef<str> + Sync, I: AsRef<str> + Sync>(
        &self,
        sobject_type: &str,
        fields: &[F],
        ids: &[I],
    ) -> Result<Vec<SObject>> {
        let body = build_retrieve_body(sobject_type, fields, ids);
        let xml = self.send(&body).await?;
        parse::parse_retrieve(&xml)
    }
}

/// Builds the `<urn:retrieve>` operation body for the given type, fields, and Ids.
///
/// Extracted so both the untyped [`retrieve`](SoapHandler::retrieve) and the
/// typed [`retrieve_typed`](SoapHandler::retrieve_typed) paths share one body
/// builder rather than duplicating the markup assembly.
pub(super) fn build_retrieve_body<F: AsRef<str>, I: AsRef<str>>(
    sobject_type: &str,
    fields: &[F],
    ids: &[I],
) -> String {
    let mut body =
        String::with_capacity(fields.len().saturating_mul(24) + ids.len().saturating_mul(48) + 64);
    body.push_str("<urn:retrieve><urn:fieldList>");
    // Write the comma-separated, escaped field list directly into the body,
    // avoiding a `Vec` + `join` intermediate allocation.
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            body.push_str(", ");
        }
        body.push_str(envelope::escape_text(field.as_ref()).as_ref());
    }
    body.push_str("</urn:fieldList><urn:sObjectType>");
    body.push_str(envelope::escape_text(sobject_type).as_ref());
    body.push_str("</urn:sObjectType>");
    for id in ids {
        body.push_str("<urn:ids>");
        body.push_str(envelope::escape_text(id.as_ref()).as_ref());
        body.push_str("</urn:ids>");
    }
    body.push_str("</urn:retrieve>");
    body
}
