//! Metadata calls for the SOAP Partner API: `describeSObject`,
//! `describeSObjects`, and `describeGlobal`.

use super::{DescribeGlobalResult, DescribeSObjectResult, SoapHandler, envelope, parse};
use crate::error::Result;

impl<A: crate::auth::Authenticator> SoapHandler<A> {
    /// Describes a single object's metadata.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn describe_sobject(&self, sobject_type: &str) -> Result<DescribeSObjectResult> {
        let mut body = String::new();
        body.push_str("<urn:describeSObject><urn:sObjectType>");
        body.push_str(&envelope::escape_text(sobject_type));
        body.push_str("</urn:sObjectType></urn:describeSObject>");
        let xml = self.send(&body).await?;
        parse::parse_describe_sobject(&xml)
    }

    /// Describes multiple objects' metadata (up to 100 per call).
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn describe_sobjects<S: AsRef<str> + Sync>(
        &self,
        sobject_types: &[S],
    ) -> Result<Vec<DescribeSObjectResult>> {
        let mut body = String::new();
        body.push_str("<urn:describeSObjects>");
        for sobject_type in sobject_types {
            body.push_str("<urn:sObjectType>");
            body.push_str(&envelope::escape_text(sobject_type.as_ref()));
            body.push_str("</urn:sObjectType>");
        }
        body.push_str("</urn:describeSObjects>");
        let xml = self.send(&body).await?;
        parse::parse_describe_sobjects(&xml)
    }

    /// Lists all objects available in the org and their high-level metadata.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn describe_global(&self) -> Result<DescribeGlobalResult> {
        let xml = self.send("<urn:describeGlobal/>").await?;
        parse::parse_describe_global(&xml)
    }
}
