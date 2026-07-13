//! Query and search calls for the SOAP Partner API: `query`, `queryMore`,
//! `queryAll`, and `search`.

use super::{QueryResult, SearchResult, SoapHandler, envelope, parse};
use crate::error::Result;

impl<A: crate::auth::Authenticator> SoapHandler<A> {
    /// Executes a SOQL query, returning the first page of results.
    ///
    /// When [`QueryResult::done`] is `false`, pass
    /// [`QueryResult::query_locator`] to [`query_more`](Self::query_more) to
    /// fetch subsequent pages.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault (for example a malformed query), or an XML parse error.
    pub async fn query(&self, soql: &str) -> Result<QueryResult> {
        let mut body = String::new();
        body.push_str("<urn:query><urn:queryString>");
        body.push_str(&envelope::escape_text(soql));
        body.push_str("</urn:queryString></urn:query>");
        let xml = self.send(&body).await?;
        parse::parse_query_result(&xml)
    }

    /// Fetches the next page of a query using a locator from a prior result.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn query_more(&self, query_locator: &str) -> Result<QueryResult> {
        let mut body = String::new();
        body.push_str("<urn:queryMore><urn:queryLocator>");
        body.push_str(&envelope::escape_text(query_locator));
        body.push_str("</urn:queryLocator></urn:queryMore>");
        let xml = self.send(&body).await?;
        parse::parse_query_result(&xml)
    }

    /// Executes a SOQL query including soft-deleted and archived records.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn query_all(&self, soql: &str) -> Result<QueryResult> {
        let mut body = String::new();
        body.push_str("<urn:queryAll><urn:queryString>");
        body.push_str(&envelope::escape_text(soql));
        body.push_str("</urn:queryString></urn:queryAll>");
        let xml = self.send(&body).await?;
        parse::parse_query_result(&xml)
    }

    /// Executes a SOSL search.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`](crate::error::ForceError) on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn search(&self, sosl: &str) -> Result<SearchResult> {
        let mut body = String::new();
        body.push_str("<urn:search><urn:searchString>");
        body.push_str(&envelope::escape_text(sosl));
        body.push_str("</urn:searchString></urn:search>");
        let xml = self.send(&body).await?;
        parse::parse_search_result(&xml)
    }
}
