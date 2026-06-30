use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use crate::types::DynamicSObject;
use std::collections::HashMap;

/// Statistics for a specific field across the queried records.
#[derive(Debug, Default, PartialEq)]
pub struct FieldStats {
    /// Number of null values encountered.
    pub null_count: usize,
    /// Total number of records processed.
    pub total_count: usize,
    /// Minimum string length encountered, if applicable.
    pub min_len: Option<usize>,
    /// Maximum string length encountered, if applicable.
    pub max_len: Option<usize>,
}

/// Utility for streaming over a dataset to compute data quality metrics.
#[derive(Debug)]
pub struct DataProfiler<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
}

impl<'a, A: Authenticator> DataProfiler<'a, A> {
    /// Creates a new `DataProfiler`.
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self { client }
    }

    /// Profiles the results of a SOQL query.
    pub async fn profile(&self, soql: &str) -> Result<HashMap<String, FieldStats>> {
        let mut stream = self.client.rest().query_stream::<DynamicSObject>(soql);
        let mut stats: HashMap<String, FieldStats> = HashMap::new();

        while let Some(record) = stream.next().await? {
            for (key, val) in record.fields {
                // Ignore nested attribute objects
                if key == "attributes" {
                    continue;
                }
                let stat = stats.entry(key).or_default();
                stat.total_count += 1;
                if val.is_null() {
                    stat.null_count += 1;
                } else if let Some(s) = val.as_str() {
                    let len = s.len();
                    if let Some(min) = stat.min_len {
                        if len < min {
                            stat.min_len = Some(len);
                        }
                    } else {
                        stat.min_len = Some(len);
                    }
                    if let Some(max) = stat.max_len {
                        if len > max {
                            stat.max_len = Some(len);
                        }
                    } else {
                        stat.max_len = Some(len);
                    }
                }
            }
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_profile_records() {
        let mock_server = MockServer::start().await;
        let query_response = json!({
            "totalSize": 2,
            "done": true,
            "records": [
                {
                    "attributes": {"type": "Account", "url": "/services/data/v60.0/sobjects/Account/001000000000001AAA"},
                    "Id": "001000000000001AAA",
                    "Name": "Acme",
                    "Website": null
                },
                {
                    "attributes": {"type": "Account", "url": "/services/data/v60.0/sobjects/Account/001000000000002AAA"},
                    "Id": "001000000000002AAA",
                    "Name": "Globex Corp",
                    "Website": "https://globex.com"
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(query_response))
            .mount(&mock_server)
            .await;

        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();
        let profiler = DataProfiler::new(&client);

        let stats = profiler
            .profile("SELECT Id, Name, Website FROM Account")
            .await
            .must();

        assert_eq!(stats.len(), 3);
        assert_eq!(stats["Name"].null_count, 0);
        assert_eq!(stats["Name"].total_count, 2);
        assert_eq!(stats["Name"].min_len, Some(4));
        assert_eq!(stats["Name"].max_len, Some(11));

        assert_eq!(stats["Website"].null_count, 1);
        assert_eq!(stats["Website"].total_count, 2);
        assert_eq!(stats["Website"].min_len, Some(18));
        assert_eq!(stats["Website"].max_len, Some(18));
    }
}
