//! Miscellaneous utility calls for the SOAP Partner API: `getUserInfo` and
//! `getServerTimestamp`.

use super::{SoapHandler, UserInfo, parse};
use crate::error::{ForceError, Result};
use chrono::{DateTime, Utc};

impl<A: crate::auth::Authenticator> SoapHandler<A> {
    /// Returns information about the current user and org.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`] on a transport failure,
    /// a SOAP fault, or an XML parse error.
    pub async fn get_user_info(&self) -> Result<UserInfo> {
        let xml = self.send("<urn:getUserInfo/>").await?;
        parse::parse_user_info(&xml)
    }

    /// Returns the current server time as a UTC timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError`] on a transport failure,
    /// a SOAP fault, an XML parse error, or if the returned timestamp cannot be
    /// parsed as an RFC 3339 datetime.
    pub async fn get_server_timestamp(&self) -> Result<DateTime<Utc>> {
        let xml = self.send("<urn:getServerTimestamp/>").await?;
        let timestamp = parse::parse_server_timestamp(&xml)?;
        DateTime::parse_from_rfc3339(&timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|err| {
                ForceError::InvalidInput(format!("invalid server timestamp '{timestamp}': {err}"))
            })
    }
}
