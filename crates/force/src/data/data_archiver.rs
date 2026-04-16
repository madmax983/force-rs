//! Data archiver for Salesforce.
//!
//! Provides `DataArchiver`, a utility for seamlessly exporting Salesforce data
//! to local disk formats (JSONL, CSV).

use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::{ForceError, Result, SerializationError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Exports a SOQL query to a JSON Lines (JSONL) file.
///
/// Each returned record will be serialized as a single JSON object per line.
///
/// # Arguments
///
/// * `client` - The Force client.
/// * `soql` - The SOQL query string.
/// * `path` - The file path to write the JSONL output.
///
/// # Returns
///
/// Returns the number of records written.
pub async fn archive_to_jsonl<A: Authenticator, T>(
    client: &ForceClient<A>,
    soql: &str,
    path: impl AsRef<Path>,
) -> Result<usize>
where
    T: DeserializeOwned + Serialize + Unpin,
{
        let mut stream = client.rest().query_stream::<T>(soql);
        let mut file = File::create(path).await?;
        let mut count = 0;

        while let Some(record) = stream.next().await? {
            let json = serde_json::to_string(&record)
                .map_err(|e| ForceError::from(SerializationError::from(e)))?;

            file.write_all(json.as_bytes()).await?;
            file.write_all(b"\n").await?;

            count += 1;
        }

        file.flush().await?;
        Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ForceClientBuilder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use std::env;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_export_to_jsonl() {
        let mock_server = MockServer::start().await;

        let query_response = json!({
            "totalSize": 2,
            "done": true,
            "records": [
                {
                    "attributes": {"type": "Account", "url": "/services/data/v60.0/sobjects/Account/001xx000000001AAA"},
                    "Id": "001xx000000001AAA",
                    "Name": "Acme"
                },
                {
                    "attributes": {"type": "Account", "url": "/services/data/v60.0/sobjects/Account/001xx000000002AAA"},
                    "Id": "001xx000000002AAA",
                    "Name": "Globex"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(query_response))
            .mount(&mock_server)
            .await;

        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = ForceClientBuilder::new()
            .authenticate(auth)
            .build()
            .await
            .must();
        let file_path = env::temp_dir().join(format!("export_{}.jsonl", std::process::id()));

        let soql = "SELECT Id, Name FROM Account";
        let count = archive_to_jsonl::<_, serde_json::Value>(&client, soql, &file_path)
            .await
            .must();

        assert_eq!(count, 2);

        // Verify file contents
        let contents = std::fs::read_to_string(&file_path).must();
        let lines: Vec<&str> = contents.lines().collect();

        assert_eq!(lines.len(), 2);

        let record1: serde_json::Value = serde_json::from_str(lines[0]).must();
        assert_eq!(record1["Name"], "Acme");

        let record2: serde_json::Value = serde_json::from_str(lines[1]).must();
        assert_eq!(record2["Name"], "Globex");

        // Cleanup
        let _ = std::fs::remove_file(file_path);
    }
}
