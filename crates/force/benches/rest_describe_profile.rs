//! One-shot profiling harness for REST `describe` (sObject metadata).
//!
//! Not a criterion benchmark: a single realistic run of the public
//! `client.rest().query::<T>()` / `query_more::<T>()` entry points, meant to be
//! executed once under `valgrind --tool=callgrind` (instruction counts) and
//! `valgrind --tool=dhat` (allocation counts).
//!
//! Workload: `describe` called for 25 custom-object-sized sObjects (150 fields
//! each, ~20% picklists with 25 values, 40 child relationships, 3 record
//! types) against an in-process mock REST endpoint. Sizes are fixed (no RNG).
//!
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --bench rest_describe_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'rest_describe_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::format_push_string,
    missing_docs
)]

use force::api::RestOperation;
use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Debug, Clone)]
struct StaticAuthenticator {
    token: String,
    instance_url: String,
}

#[async_trait::async_trait]
impl Authenticator for StaticAuthenticator {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new(self.token.clone().into()),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "profile-sig".to_string(),
            expires_in: Some(7200),
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}

const OBJECTS: usize = 25;
const FIELDS: usize = 150;

fn field_json(i: usize) -> String {
    let (ty, picklist) = match i % 5 {
        0 => ("picklist", true),
        1 => ("reference", false),
        2 => ("double", false),
        3 => ("boolean", false),
        _ => ("string", false),
    };
    let pl = if picklist {
        let vals: Vec<String> = (0..25)
            .map(|v| format!("{{\"active\":true,\"defaultValue\":false,\"label\":\"Option Label {v}\",\"validFor\":null,\"value\":\"Option_Value_{v}\"}}"))
            .collect();
        format!("[{}]", vals.join(","))
    } else {
        "[]".to_string()
    };
    let refs = if ty == "reference" {
        "[\"User\"]"
    } else {
        "[]"
    };
    format!(
        "{{\"aggregatable\":false,\"autoNumber\":false,\"byteLength\":765,\"calculated\":false,\"calculatedFormula\":null,\"cascadeDelete\":false,\"caseSensitive\":false,\"compoundFieldName\":null,\"controllerName\":null,\"createable\":true,\"custom\":true,\"defaultValue\":null,\"defaultValueFormula\":null,\"defaultedOnCreate\":false,\"dependentPicklist\":false,\"deprecatedAndHidden\":false,\"digits\":0,\"displayLocationInDecimal\":false,\"encrypted\":false,\"externalId\":false,\"extraTypeInfo\":null,\"filterable\":true,\"filteredLookupInfo\":null,\"formulaTreatBlanksAs\":\"BlankAsZero\",\"groupable\":true,\"highScaleNumber\":false,\"htmlFormatted\":false,\"idLookup\":false,\"inlineHelpText\":\"Help text for custom field number {i}\",\"label\":\"Custom Field Number {i}\",\"length\":255,\"mask\":null,\"maskType\":null,\"name\":\"Custom_Field_{i}__c\",\"nameField\":false,\"namePointing\":false,\"nillable\":true,\"permissionable\":true,\"picklistValues\":{pl},\"polymorphicForeignKey\":false,\"precision\":0,\"queryByDistance\":false,\"referenceTargetField\":null,\"referenceTo\":{refs},\"relationshipName\":null,\"relationshipOrder\":null,\"restrictedDelete\":false,\"restrictedPicklist\":false,\"scale\":0,\"searchPrefixesSupported\":false,\"soapType\":\"xsd:string\",\"sortable\":true,\"type\":\"{ty}\",\"unique\":false,\"updateable\":true,\"writeRequiresMasterRead\":false}}"
    )
}

fn describe_body(obj: usize) -> String {
    let fields: Vec<String> = (0..FIELDS).map(field_json).collect();
    let children: Vec<String> = (0..40)
        .map(|c| format!("{{\"cascadeDelete\":false,\"childSObject\":\"Child_{c}__c\",\"deprecatedAndHidden\":false,\"field\":\"Parent_{obj}__c\",\"relationshipName\":\"Children{c}__r\",\"restrictedDelete\":false}}"))
        .collect();
    let rts: Vec<String> = (0..3)
        .map(|r| format!("{{\"active\":true,\"defaultRecordTypeMapping\":{},\"developerName\":\"RT{r}\",\"master\":{},\"name\":\"Record Type {r}\",\"recordTypeId\":\"012xx00000000{r}AAA\",\"urls\":{{\"layout\":\"/services/data/v67.0/sobjects/Obj{obj}__c/describe/layouts/012xx00000000{r}AAA\"}}}}", r == 0, r == 0))
        .collect();
    format!(
        "{{\"activateable\":false,\"createable\":true,\"custom\":true,\"customSetting\":false,\"deletable\":true,\"deprecatedAndHidden\":false,\"feedEnabled\":true,\"hasSubtypes\":false,\"isSubtype\":false,\"keyPrefix\":\"a0{obj:02}\",\"label\":\"Object {obj}\",\"labelPlural\":\"Objects {obj}\",\"layoutable\":true,\"mergeable\":false,\"mruEnabled\":true,\"name\":\"Obj{obj}__c\",\"queryable\":true,\"replicateable\":true,\"retrieveable\":true,\"searchable\":true,\"triggerable\":true,\"undeletable\":true,\"updateable\":true,\"fields\":[{}],\"childRelationships\":[{}],\"recordTypeInfos\":[{}],\"urls\":{{\"sobject\":\"/services/data/v67.0/sobjects/Obj{obj}__c\",\"describe\":\"/services/data/v67.0/sobjects/Obj{obj}__c/describe\"}}}}",
        fields.join(","),
        children.join(","),
        rts.join(",")
    )
}

#[tokio::main]
async fn main() {
    let server = MockServer::start().await;
    for obj in 0..OBJECTS {
        Mock::given(method("GET"))
            .and(path(format!(
                "/services/data/v67.0/sobjects/Obj{obj}__c/describe"
            )))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Type", "application/json")
                    .set_body_string(describe_body(obj)),
            )
            .mount(&server)
            .await;
    }
    let auth = StaticAuthenticator {
        token: "profile-token".to_string(),
        instance_url: server.uri(),
    };
    let client = builder().authenticate(auth).build().await.expect("client");
    let rest = client.rest();

    let mut total = 0usize;
    for obj in 0..OBJECTS {
        let d = rest
            .describe(&format!("Obj{obj}__c"))
            .await
            .expect("describe");
        total += d.fields.len();
    }
    println!("total_fields={total}");
}
