//! Data Profiler for analyzing data quality and fill rates.

use crate::api::rest_operation::RestOperation;
use crate::api::soql::SoqlQueryBuilder;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use crate::types::DynamicSObject;
use futures::StreamExt;
use std::collections::HashMap;

/// A profile for a single field, detailing its population statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldProfile {
    /// The API name of the field.
    pub name: String,
    /// Total number of records sampled.
    pub total_sampled: usize,
    /// Number of records where this field was not null.
    pub populated_count: usize,
    /// Number of records where this field was null.
    pub null_count: usize,
    /// The percentage of populated records (0.0 to 1.0).
    pub fill_rate: f64,
}

/// A data quality profile for an SObject.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectProfile {
    /// The API name of the SObject.
    pub sobject: String,
    /// Total number of records sampled in this profile.
    pub total_sampled: usize,
    /// The profile of each individual field.
    pub fields: HashMap<String, FieldProfile>,
}

/// Utility for analyzing data quality directly from a Salesforce org.
#[derive(Debug)]
pub struct DataProfiler<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
}

impl<'a, A: Authenticator> DataProfiler<'a, A> {
    /// Creates a new data profiler.
    ///
    /// # Arguments
    ///
    /// * `client` - The Force client.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self { client }
    }

    /// Generates a data profile for the given SObject by sampling real records.
    ///
    /// This method fetches the SObject describe metadata, constructs a SOQL query
    /// requesting all queryable fields, and processes up to `sample_size` records
    /// to calculate fill rates and null counts.
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject (e.g., "Account").
    /// * `sample_size` - The maximum number of records to sample.
    ///
    /// # Errors
    ///
    /// Returns an error if the describe call fails, SOQL generation fails, or
    /// the data query stream encounters an HTTP error.
    pub async fn profile(&self, sobject: &str, sample_size: u32) -> Result<ObjectProfile> {
        let describe = self.client.rest().describe(sobject).await?;

        let mut builder = SoqlQueryBuilder::from_describe(&describe);
        builder = builder.limit(sample_size);
        let soql = builder.try_build()?;

        let mut stream = self.client.rest().query_stream::<DynamicSObject>(&soql);
        let mut stream = std::pin::pin!(stream);

        let mut total_sampled = 0;
        let mut field_profiles: HashMap<String, FieldProfile> = describe
            .fields
            .iter()
            .map(|f| {
                (
                    f.name.clone(),
                    FieldProfile {
                        name: f.name.clone(),
                        total_sampled: 0,
                        populated_count: 0,
                        null_count: 0,
                        fill_rate: 0.0,
                    },
                )
            })
            .collect();

        while let Some(record_result) = stream.next().await {
            let record = record_result?;
            total_sampled += 1;
            for (field_name, profile) in &mut field_profiles {
                profile.total_sampled += 1;
                if let Some(val) = record.get_field(field_name) {
                    if val.is_null() {
                        profile.null_count += 1;
                    } else {
                        profile.populated_count += 1;
                    }
                } else {
                    profile.null_count += 1;
                }
            }
        }

        for profile in field_profiles.values_mut() {
            if profile.total_sampled > 0 {
                profile.fill_rate =
                    (profile.populated_count as f64) / (profile.total_sampled as f64);
            }
        }

        Ok(ObjectProfile {
            sobject: sobject.to_string(),
            total_sampled,
            fields: field_profiles,
        })
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
    async fn test_data_profiler() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let describe_json = json!({
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Id", "type": "id", "label": "Account ID", "createable": false,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:id",
                    "sortable": true, "unique": true, "updateable": false, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "Name", "type": "string", "label": "Account Name", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false, "nillable": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                },
                {
                    "name": "Industry", "type": "string", "label": "Industry", "createable": true,
                    "autoNumber": false, "calculated": false,
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false, "custom": false,
                    "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": false, "namePointing": false, "nillable": true,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
                    "referenceTo": []
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        let query_response = json!({
            "totalSize": 3,
            "done": true,
            "records": [
                {
                    "attributes": {"type": "Account", "url": "/services/data/v67.0/sobjects/Account/001xx000000001AAA"},
                    "Id": "001xx000000001AAA",
                    "Name": "Acme",
                    "Industry": "Technology"
                },
                {
                    "attributes": {"type": "Account", "url": "/services/data/v67.0/sobjects/Account/001xx000000002AAA"},
                    "Id": "001xx000000002AAA",
                    "Name": "Globex",
                    "Industry": null
                },
                {
                    "attributes": {"type": "Account", "url": "/services/data/v67.0/sobjects/Account/001xx000000003AAA"},
                    "Id": "001xx000000003AAA",
                    "Name": "Initech"
                    // Industry omitted intentionally to test missing field handling
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(query_response))
            .mount(&mock_server)
            .await;

        let profiler = DataProfiler::new(&client);
        let profile = profiler.profile("Account", 10).await.must();

        assert_eq!(profile.sobject, "Account");
        assert_eq!(profile.total_sampled, 3);

        let id_profile = profile.fields.get("Id").must();
        assert_eq!(id_profile.populated_count, 3);
        assert_eq!(id_profile.fill_rate, 1.0);

        let name_profile = profile.fields.get("Name").must();
        assert_eq!(name_profile.populated_count, 3);
        assert_eq!(name_profile.fill_rate, 1.0);

        let ind_profile = profile.fields.get("Industry").must();
        assert_eq!(ind_profile.populated_count, 1);
        assert_eq!(ind_profile.null_count, 2); // one explicitly null, one omitted
        assert!((ind_profile.fill_rate - 0.3333333333333333).abs() < f64::EPSILON);
    }
}
