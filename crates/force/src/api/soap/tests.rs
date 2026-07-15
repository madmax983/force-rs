//! Tests for the SOAP Partner API handler.
// XML fixtures use raw strings uniformly for readability; some happen not to
// contain quotes, which is fine here.
#![allow(clippy::needless_raw_string_hashes)]

use super::*;
use crate::test_utils::mock_auth::MockAuthenticator;
use crate::test_utils::must::Must;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn setup() -> (MockServer, crate::client::ForceClient<MockAuthenticator>) {
    let server = MockServer::start().await;
    let auth = MockAuthenticator::new("test_token", &server.uri());
    let client = crate::client::builder()
        .authenticate(auth)
        .build()
        .await
        .must();
    (server, client)
}

/// Mounts a single POST responder on the SOAP endpoint returning the given XML.
async fn mount_soap(server: &MockServer, status: u16, xml: &str) {
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .respond_with(
            ResponseTemplate::new(status)
                .insert_header("Content-Type", "text/xml; charset=utf-8")
                .set_body_string(xml),
        )
        .mount(server)
        .await;
}

const NS: &str = concat!(
    r#" xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/""#,
    r#" xmlns="urn:partner.soap.sforce.com""#,
    r#" xmlns:sf="urn:sobject.partner.soap.sforce.com""#,
    r#" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#,
);

fn envelope(inner: &str) -> String {
    format!("<soapenv:Envelope{NS}><soapenv:Body>{inner}</soapenv:Body></soapenv:Envelope>")
}

// ── Handler construction ─────────────────────────────────────────────

#[tokio::test]
async fn test_handler_is_cloneable_and_debug() {
    let (_server, client) = setup().await;
    let handler = client.soap();
    let cloned = handler.clone();
    assert_eq!(format!("{handler:?}"), format!("{cloned:?}"));
    assert!(format!("{handler:?}").contains("SoapHandler"));
}

#[tokio::test]
async fn test_soap_url_strips_version_prefix() {
    let (_server, client) = setup().await;
    let handler = client.soap();
    let url = handler.soap_url("https://na1.salesforce.com");
    assert_eq!(url, "https://na1.salesforce.com/services/Soap/u/67.0");
    // Trailing slash is normalized away.
    let url2 = handler.soap_url("https://na1.salesforce.com/");
    assert_eq!(url2, "https://na1.salesforce.com/services/Soap/u/67.0");
}

// ── Envelope construction (unit) ─────────────────────────────────────

#[test]
fn test_build_envelope_contains_session_header() {
    let xml = envelope::build_envelope("SECRET_TOKEN", Some("force-rs"), "<urn:getUserInfo/>");
    assert!(xml.contains("<urn:SessionHeader><urn:sessionId>SECRET_TOKEN</urn:sessionId>"));
    assert!(xml.contains("<urn:CallOptions><urn:client>force-rs</urn:client></urn:CallOptions>"));
    assert!(xml.contains("<soapenv:Body><urn:getUserInfo/></soapenv:Body>"));
    // Namespaces declared on the envelope.
    assert!(xml.contains("xmlns:urn=\"urn:partner.soap.sforce.com\""));
    assert!(xml.contains("xmlns:sf=\"urn:sobject.partner.soap.sforce.com\""));
}

#[test]
fn test_build_envelope_without_client() {
    let xml = envelope::build_envelope("tok", None, "<urn:describeGlobal/>");
    assert!(!xml.contains("CallOptions"));
}

#[test]
fn test_serialize_sobject_escapes_values() {
    let obj = SObject::new("Account")
        .with_field("Name", r#"A & B <x> "q""#)
        .with_null_field("Website");
    let mut out = String::new();
    envelope::serialize_sobject(&mut out, &obj);
    assert!(out.contains("<sf:type>Account</sf:type>"));
    // The dangerous characters must be escaped in the value.
    assert!(out.contains("<sf:Name>A &amp; B &lt;x&gt; &quot;q&quot;</sf:Name>"));
    assert!(!out.contains("<x>"));
    assert!(out.contains("<sf:fieldsToNull>Website</sf:fieldsToNull>"));
}

#[test]
fn test_serialize_sobject_skips_none_fields() {
    let mut obj = SObject::new("Contact");
    obj.fields.push(("Phantom".to_string(), None));
    obj.set_field("LastName", "Doe");
    let mut out = String::new();
    envelope::serialize_sobject(&mut out, &obj);
    assert!(!out.contains("Phantom"));
    assert!(out.contains("<sf:LastName>Doe</sf:LastName>"));
}

// ── escape_text (unit) ───────────────────────────────────────────────

#[test]
fn test_escape_text_no_escape_borrows() {
    // The common fast path (identifiers/Ids/plain values) must not allocate:
    // `escape` returns a borrowed Cow when nothing needs escaping.
    assert!(matches!(
        envelope::escape_text("001xx0000000001AAA"),
        std::borrow::Cow::Borrowed(_)
    ));
    assert!(matches!(
        envelope::escape_text("Plain Name 123"),
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn test_escape_text_escapes_all_markup_chars() {
    // Each character that carries XML meaning must be escaped, and the escaping
    // path returns an owned Cow.
    let escaped = envelope::escape_text(r#"< > & ' ""#);
    assert!(matches!(escaped, std::borrow::Cow::Owned(_)));
    assert_eq!(escaped.as_ref(), "&lt; &gt; &amp; &apos; &quot;");
}

// ── parse_fault (unit) ───────────────────────────────────────────────

#[test]
fn test_parse_fault_none_on_query_success() {
    // A normal, fault-free query response must short-circuit to None without
    // reporting a spurious fault.
    let body = envelope(
        r#"<queryResponse><result xsi:type="QueryResult">
            <done>true</done>
            <records xsi:type="sf:sObject">
                <sf:type>Account</sf:type>
                <sf:Id>001xx0000000001AAA</sf:Id>
                <sf:Name>Acme</sf:Name>
            </records>
            <size>1</size>
        </result></queryResponse>"#,
    );
    assert!(fault::parse_fault(&body).is_none());
}

#[test]
fn test_parse_fault_still_parses_a_real_fault() {
    let body = envelope(
        r#"<soapenv:Fault>
            <faultcode>sf:INVALID_SESSION_ID</faultcode>
            <faultstring>INVALID_SESSION_ID: Invalid Session</faultstring>
            <detail><sf:UnexpectedErrorFault>
                <sf:exceptionCode>INVALID_SESSION_ID</sf:exceptionCode>
            </sf:UnexpectedErrorFault></detail>
        </soapenv:Fault>"#,
    );
    let fault = fault::parse_fault(&body).must();
    assert!(fault.is_invalid_session());
    assert_eq!(fault.exception_code.as_deref(), Some("INVALID_SESSION_ID"));
}

// ── build_retrieve_body (unit) ───────────────────────────────────────

#[test]
fn test_build_retrieve_body_matches_known_good() {
    let body = crud::build_retrieve_body("Account", &["Name", "Industry"], &["001AAA", "001BBB"]);
    assert_eq!(
        body,
        "<urn:retrieve><urn:fieldList>Name, Industry</urn:fieldList>\
<urn:sObjectType>Account</urn:sObjectType>\
<urn:ids>001AAA</urn:ids><urn:ids>001BBB</urn:ids></urn:retrieve>"
    );
}

// ── query ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_query_success() {
    let (server, client) = setup().await;
    let inner = r#"<queryResponse><result xsi:type="QueryResult">
        <done>false</done>
        <queryLocator>01gLOC-2000</queryLocator>
        <records xsi:type="sf:sObject">
            <sf:type>Account</sf:type>
            <sf:Id>001AAA</sf:Id>
            <sf:Id>001AAA</sf:Id>
            <sf:Name>Acme Corp</sf:Name>
            <sf:NumberOfEmployees>100</sf:NumberOfEmployees>
        </records>
        <records xsi:type="sf:sObject">
            <sf:type>Account</sf:type>
            <sf:Id>001BBB</sf:Id>
            <sf:Id>001BBB</sf:Id>
            <sf:Name>Beta LLC</sf:Name>
            <sf:NumberOfEmployees xsi:nil="true"/>
        </records>
        <size>1723</size>
    </result></queryResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let result = client
        .soap()
        .query("SELECT Id, Name FROM Account")
        .await
        .must();

    assert!(!result.done);
    assert_eq!(result.query_locator.as_deref(), Some("01gLOC-2000"));
    assert_eq!(result.size, 1723);
    assert_eq!(result.records.len(), 2);

    let first = &result.records[0];
    assert_eq!(first.sobject_type, "Account");
    // The duplicated <sf:Id> is de-duplicated: exactly one Id field.
    assert_eq!(first.fields.iter().filter(|(n, _)| n == "Id").count(), 1);
    assert_eq!(first.id(), Some("001AAA"));
    assert_eq!(first.get("Name"), Some("Acme Corp"));
    assert_eq!(first.get("NumberOfEmployees"), Some("100"));

    let second = &result.records[1];
    // xsi:nil field is stored with a None value.
    assert_eq!(second.get("NumberOfEmployees"), None);
    assert!(
        second
            .fields
            .iter()
            .any(|(n, v)| n == "NumberOfEmployees" && v.is_none())
    );
}

#[tokio::test]
async fn test_query_sends_soql_in_body() {
    let (server, client) = setup().await;
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .and(header("SOAPAction", "\"\""))
        .and(body_string_contains(
            "<urn:queryString>SELECT Id FROM Account</urn:queryString>",
        ))
        .and(body_string_contains(
            "<urn:sessionId>test_token</urn:sessionId>",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(envelope(
            r#"<queryResponse><result><done>true</done><size>0</size></result></queryResponse>"#,
        )))
        .mount(&server)
        .await;

    let result = client.soap().query("SELECT Id FROM Account").await.must();
    assert!(result.done);
    assert_eq!(result.size, 0);
    assert!(result.records.is_empty());
}

#[tokio::test]
async fn test_query_more_success() {
    let (server, client) = setup().await;
    let inner = r#"<queryMoreResponse><result>
        <done>true</done>
        <records xsi:type="sf:sObject"><sf:type>Account</sf:type><sf:Id>001CCC</sf:Id></records>
        <size>1</size>
    </result></queryMoreResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let result = client.soap().query_more("01gLOC-2000").await.must();
    assert!(result.done);
    assert_eq!(result.query_locator, None);
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].id(), Some("001CCC"));
}

#[tokio::test]
async fn test_query_all_success() {
    let (server, client) = setup().await;
    let inner =
        r#"<queryAllResponse><result><done>true</done><size>0</size></result></queryAllResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;
    let result = client
        .soap()
        .query_all("SELECT Id FROM Account")
        .await
        .must();
    assert!(result.done);
}

// ── create / update ──────────────────────────────────────────────────

#[tokio::test]
async fn test_create_partial_failure() {
    let (server, client) = setup().await;
    let inner = r#"<createResponse>
        <result><id>001AAA</id><success>true</success></result>
        <result>
            <success>false</success>
            <errors>
                <statusCode>REQUIRED_FIELD_MISSING</statusCode>
                <message>Required fields are missing: [Name]</message>
                <fields>Name</fields>
            </errors>
        </result>
    </createResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let records = vec![
        SObject::new("Account").with_field("Name", "Acme"),
        SObject::new("Account"),
    ];
    let results = client.soap().create(&records).await.must();

    assert_eq!(results.len(), 2);
    assert!(results[0].success);
    assert_eq!(results[0].id.as_deref(), Some("001AAA"));
    assert!(results[0].errors.is_empty());

    assert!(!results[1].success);
    assert_eq!(results[1].id, None);
    assert_eq!(results[1].errors.len(), 1);
    assert_eq!(results[1].errors[0].status_code, "REQUIRED_FIELD_MISSING");
    assert_eq!(results[1].errors[0].fields, vec!["Name".to_string()]);
}

#[tokio::test]
async fn test_create_sends_sobject_body() {
    let (server, client) = setup().await;
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .and(body_string_contains(
            "<urn:create><urn:sObjects><sf:type>Account</sf:type>",
        ))
        .and(body_string_contains("<sf:Name>Acme</sf:Name>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(envelope(
            r#"<createResponse><result><id>001AAA</id><success>true</success></result></createResponse>"#,
        )))
        .mount(&server)
        .await;
    let records = vec![SObject::new("Account").with_field("Name", "Acme")];
    let results = client.soap().create(&records).await.must();
    assert!(results[0].success);
}

#[tokio::test]
async fn test_update_success() {
    let (server, client) = setup().await;
    let inner = r#"<updateResponse><result><id>001AAA</id><success>true</success></result></updateResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;
    let records = vec![
        SObject::new("Account")
            .with_field("Id", "001AAA")
            .with_field("Name", "Renamed"),
    ];
    let results = client.soap().update(&records).await.must();
    assert_eq!(results.len(), 1);
    assert!(results[0].success);
    assert_eq!(results[0].id.as_deref(), Some("001AAA"));
}

// ── upsert ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_upsert_success() {
    let (server, client) = setup().await;
    let inner = r#"<upsertResponse>
        <result><created>true</created><id>001NEW</id><success>true</success></result>
        <result><created>false</created><id>001OLD</id><success>true</success></result>
    </upsertResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let records = vec![
        SObject::new("Account").with_field("Ext__c", "E1"),
        SObject::new("Account").with_field("Ext__c", "E2"),
    ];
    let results = client.soap().upsert("Ext__c", &records).await.must();
    assert_eq!(results.len(), 2);
    assert!(results[0].created);
    assert_eq!(results[0].id.as_deref(), Some("001NEW"));
    assert!(!results[1].created);
    assert!(results[1].success);
}

#[tokio::test]
async fn test_upsert_sends_external_id_field() {
    let (server, client) = setup().await;
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .and(body_string_contains(
            "<urn:externalIDFieldName>Ext__c</urn:externalIDFieldName>",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(envelope(
            r#"<upsertResponse><result><created>true</created><id>001NEW</id><success>true</success></result></upsertResponse>"#,
        )))
        .mount(&server)
        .await;
    let records = vec![SObject::new("Account").with_field("Ext__c", "E1")];
    let results = client.soap().upsert("Ext__c", &records).await.must();
    assert!(results[0].created);
}

// ── delete ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_delete_success() {
    let (server, client) = setup().await;
    let inner = r#"<deleteResponse>
        <result><id>001AAA</id><success>true</success></result>
        <result><id>001BBB</id><success>false</success>
            <errors><statusCode>ENTITY_IS_DELETED</statusCode><message>entity is deleted</message></errors>
        </result>
    </deleteResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let results = client.soap().delete(&["001AAA", "001BBB"]).await.must();
    assert_eq!(results.len(), 2);
    assert!(results[0].success);
    assert_eq!(results[0].id.as_deref(), Some("001AAA"));
    assert!(!results[1].success);
    assert_eq!(results[1].errors[0].status_code, "ENTITY_IS_DELETED");
}

#[tokio::test]
async fn test_delete_accepts_owned_strings() {
    let (server, client) = setup().await;
    let inner = r#"<deleteResponse><result><id>001AAA</id><success>true</success></result></deleteResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;
    let ids = vec!["001AAA".to_string()];
    let results = client.soap().delete(&ids).await.must();
    assert!(results[0].success);
}

// ── retrieve ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_retrieve_success() {
    let (server, client) = setup().await;
    let inner = r#"<retrieveResponse>
        <result xsi:type="sf:sObject"><sf:type>Account</sf:type><sf:Id>001AAA</sf:Id><sf:Name>Acme</sf:Name></result>
        <result xsi:nil="true"/>
    </retrieveResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let records = client
        .soap()
        .retrieve("Account", &["Id", "Name"], &["001AAA", "001MISSING"])
        .await
        .must();
    // The nil (not-found) slot is skipped.
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id(), Some("001AAA"));
    assert_eq!(records[0].get("Name"), Some("Acme"));
}

// ── search ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_search_success() {
    let (server, client) = setup().await;
    let inner = r#"<searchResponse><result>
        <searchRecords>
            <record xsi:type="sf:sObject"><sf:type>Account</sf:type><sf:Id>001AAA</sf:Id><sf:Name>Acme</sf:Name></record>
        </searchRecords>
        <searchRecords>
            <record xsi:type="sf:sObject"><sf:type>Contact</sf:type><sf:Id>003XXX</sf:Id><sf:Name>Jane</sf:Name></record>
        </searchRecords>
    </result></searchResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let result = client
        .soap()
        .search("FIND {Acme} RETURNING Account(Id, Name), Contact(Id, Name)")
        .await
        .must();
    assert_eq!(result.records.len(), 2);
    assert_eq!(result.records[0].sobject_type, "Account");
    assert_eq!(result.records[1].sobject_type, "Contact");
    assert_eq!(result.records[1].get("Name"), Some("Jane"));
}

// ── describe ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_describe_sobject_success() {
    let (server, client) = setup().await;
    let inner = r#"<describeSObjectResponse><result>
        <name>Account</name>
        <label>Account</label>
        <labelPlural>Accounts</labelPlural>
        <keyPrefix>001</keyPrefix>
        <custom>false</custom>
        <createable>true</createable>
        <updateable>true</updateable>
        <deletable>true</deletable>
        <queryable>true</queryable>
        <fields>
            <name>Name</name>
            <label>Account Name</label>
            <type>string</type>
            <length>255</length>
            <nillable>false</nillable>
            <custom>false</custom>
        </fields>
        <fields>
            <name>Industry</name>
            <type>picklist</type>
            <nillable>true</nillable>
            <picklistValues><value>Banking</value><label>Banking</label><active>true</active></picklistValues>
            <picklistValues><value>Retail</value><label>Retail</label><active>true</active></picklistValues>
        </fields>
    </result></describeSObjectResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let describe = client.soap().describe_sobject("Account").await.must();
    assert_eq!(describe.name, "Account");
    assert_eq!(describe.label_plural.as_deref(), Some("Accounts"));
    assert_eq!(describe.key_prefix.as_deref(), Some("001"));
    assert!(!describe.custom);
    assert!(describe.createable);
    assert_eq!(describe.fields.len(), 2);
    assert_eq!(describe.fields[0].name, "Name");
    assert_eq!(describe.fields[0].field_type.as_deref(), Some("string"));
    assert_eq!(describe.fields[0].length, Some(255));
    assert_eq!(describe.fields[1].picklist_values.len(), 2);
    assert_eq!(describe.fields[1].picklist_values[0].value, "Banking");
}

#[tokio::test]
async fn test_describe_sobjects_success() {
    let (server, client) = setup().await;
    let inner = r#"<describeSObjectsResponse>
        <result><name>Account</name><label>Account</label><custom>false</custom></result>
        <result><name>Contact</name><label>Contact</label><custom>false</custom></result>
    </describeSObjectsResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let results = client
        .soap()
        .describe_sobjects(&["Account", "Contact"])
        .await
        .must();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "Account");
    assert_eq!(results[1].name, "Contact");
}

#[tokio::test]
async fn test_describe_global_success() {
    let (server, client) = setup().await;
    let inner = r#"<describeGlobalResponse><result>
        <encoding>UTF-8</encoding>
        <maxBatchSize>200</maxBatchSize>
        <sobjects><name>Account</name><label>Account</label><keyPrefix>001</keyPrefix><custom>false</custom><createable>true</createable><queryable>true</queryable><updateable>true</updateable><deletable>true</deletable></sobjects>
        <sobjects><name>My__c</name><label>My Object</label><keyPrefix>a00</keyPrefix><custom>true</custom><createable>true</createable><queryable>true</queryable><updateable>true</updateable><deletable>true</deletable></sobjects>
    </result></describeGlobalResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let result = client.soap().describe_global().await.must();
    assert_eq!(result.encoding.as_deref(), Some("UTF-8"));
    assert_eq!(result.max_batch_size, Some(200));
    assert_eq!(result.sobjects.len(), 2);
    assert!(!result.sobjects[0].custom);
    assert!(result.sobjects[1].custom);
    assert_eq!(result.sobjects[1].name, "My__c");
}

// ── getUserInfo / getServerTimestamp ─────────────────────────────────

#[tokio::test]
async fn test_get_user_info_success() {
    let (server, client) = setup().await;
    let inner = r#"<getUserInfoResponse><result>
        <userId>005AAA</userId>
        <userFullName>Jane Admin</userFullName>
        <userEmail>jane@example.com</userEmail>
        <userName>jane@example.com.org</userName>
        <organizationId>00DAAA</organizationId>
        <organizationName>Acme</organizationName>
        <profileId>00eAAA</profileId>
        <sessionSecondsValid>7200</sessionSecondsValid>
        <orgHasPersonAccounts>false</orgHasPersonAccounts>
    </result></getUserInfoResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let info = client.soap().get_user_info().await.must();
    assert_eq!(info.user_id, "005AAA");
    assert_eq!(info.user_full_name, "Jane Admin");
    assert_eq!(info.user_email, "jane@example.com");
    assert_eq!(info.organization_id, "00DAAA");
    assert_eq!(info.profile_id.as_deref(), Some("00eAAA"));
    assert_eq!(info.session_seconds_valid, Some(7200));
    assert_eq!(info.org_has_person_accounts, Some(false));
}

#[tokio::test]
async fn test_get_server_timestamp_success() {
    let (server, client) = setup().await;
    let inner = r#"<getServerTimestampResponse><result><timestamp>2026-07-13T18:22:41.000Z</timestamp></result></getServerTimestampResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let ts = client.soap().get_server_timestamp().await.must();
    assert_eq!(ts.to_rfc3339(), "2026-07-13T18:22:41+00:00");
}

// ── faults ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_soap_fault_returns_error() {
    let (server, client) = setup().await;
    let fault = r#"<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:sf="urn:fault.partner.soap.sforce.com" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
        <soapenv:Body><soapenv:Fault>
            <faultcode>sf:MALFORMED_QUERY</faultcode>
            <faultstring>MALFORMED_QUERY: unexpected token: FROM</faultstring>
            <detail>
                <sf:MalformedQueryFault xsi:type="sf:MalformedQueryFault">
                    <sf:exceptionCode>MALFORMED_QUERY</sf:exceptionCode>
                    <sf:exceptionMessage>unexpected token: FROM</sf:exceptionMessage>
                </sf:MalformedQueryFault>
            </detail>
        </soapenv:Fault></soapenv:Body>
    </soapenv:Envelope>"#;
    mount_soap(&server, 500, fault).await;

    let result = client.soap().query("SELECT FROM").await;
    let Err(crate::error::ForceError::Soap(fault)) = result else {
        panic!("expected ForceError::Soap, got {result:?}");
    };
    assert_eq!(fault.fault_code, "sf:MALFORMED_QUERY");
    assert!(fault.fault_string.contains("unexpected token"));
    assert_eq!(fault.exception_code.as_deref(), Some("MALFORMED_QUERY"));
    assert!(!fault.is_invalid_session());
}

#[tokio::test]
async fn test_invalid_session_refreshes_and_retries_once() {
    let (server, client) = setup().await;

    let fault = r#"<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:sf="urn:fault.partner.soap.sforce.com">
        <soapenv:Body><soapenv:Fault>
            <faultcode>sf:INVALID_SESSION_ID</faultcode>
            <faultstring>INVALID_SESSION_ID: Invalid Session ID found in SessionHeader</faultstring>
            <detail><sf:UnexpectedErrorFault><sf:exceptionCode>INVALID_SESSION_ID</sf:exceptionCode></sf:UnexpectedErrorFault></detail>
        </soapenv:Fault></soapenv:Body>
    </soapenv:Envelope>"#;

    // First call: HTTP 500 INVALID_SESSION_ID fault (served exactly once).
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .respond_with(ResponseTemplate::new(500).set_body_string(fault))
        .up_to_n_times(1)
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;

    // Second call after refresh: success.
    let ok = envelope(
        r#"<queryResponse><result><done>true</done><size>1</size>
            <records xsi:type="sf:sObject"><sf:type>Account</sf:type><sf:Id>001AAA</sf:Id></records>
        </result></queryResponse>"#,
    );
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ok))
        .with_priority(2)
        .expect(1)
        .mount(&server)
        .await;

    let result = client.soap().query("SELECT Id FROM Account").await.must();
    assert!(result.done);
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].id(), Some("001AAA"));
    // Expectations (each mock hit exactly once) are verified on server drop.
}

#[tokio::test]
async fn test_invalid_session_twice_returns_error() {
    let (server, client) = setup().await;
    let fault = r#"<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:sf="urn:fault.partner.soap.sforce.com">
        <soapenv:Body><soapenv:Fault>
            <faultcode>sf:INVALID_SESSION_ID</faultcode>
            <faultstring>INVALID_SESSION_ID: Invalid Session ID</faultstring>
            <detail><sf:UnexpectedErrorFault><sf:exceptionCode>INVALID_SESSION_ID</sf:exceptionCode></sf:UnexpectedErrorFault></detail>
        </soapenv:Fault></soapenv:Body>
    </soapenv:Envelope>"#;
    // Every call faults; the handler retries once then gives up.
    mount_soap(&server, 500, fault).await;

    let result = client.soap().query("SELECT Id FROM Account").await;
    assert!(matches!(result, Err(crate::error::ForceError::Soap(_))));
}

// ── typed convenience layer ──────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct TypedAccount {
    #[serde(rename = "Id", skip_serializing_if = "Option::is_none", default)]
    id: Option<String>,
    #[serde(rename = "Name")]
    name: String,
    // No `skip_serializing_if`: a `None` serializes to JSON null, which the
    // bridge maps to an explicit `fieldsToNull` entry.
    #[serde(rename = "Website", default)]
    website: Option<String>,
}

#[tokio::test]
async fn test_query_typed_auto_paginates_two_pages() {
    let (server, client) = setup().await;

    // Page 1 (query): done=false with a locator, one record.
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .and(body_string_contains("<urn:query>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(envelope(
            r#"<queryResponse><result>
                <done>false</done>
                <queryLocator>01gLOC-PAGE2</queryLocator>
                <records xsi:type="sf:sObject"><sf:type>Account</sf:type><sf:Id>001AAA</sf:Id><sf:Name>Acme</sf:Name></records>
                <size>2</size>
            </result></queryResponse>"#,
        )))
        .mount(&server)
        .await;

    // Page 2 (queryMore): done=true, one record.
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .and(body_string_contains(
            "<urn:queryMore><urn:queryLocator>01gLOC-PAGE2</urn:queryLocator>",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(envelope(
            r#"<queryMoreResponse><result>
                <done>true</done>
                <records xsi:type="sf:sObject"><sf:type>Account</sf:type><sf:Id>001BBB</sf:Id><sf:Name>Beta</sf:Name></records>
                <size>2</size>
            </result></queryMoreResponse>"#,
        )))
        .mount(&server)
        .await;

    let accounts: Vec<TypedAccount> = client
        .soap()
        .query_typed("SELECT Id, Name FROM Account")
        .await
        .must();

    // Both pages are combined into one typed vector.
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[0].id.as_deref(), Some("001AAA"));
    assert_eq!(accounts[0].name, "Acme");
    assert_eq!(accounts[1].id.as_deref(), Some("001BBB"));
    assert_eq!(accounts[1].name, "Beta");
}

#[tokio::test]
async fn test_query_typed_page_returns_first_page_and_locator() {
    let (server, client) = setup().await;
    let inner = r#"<queryResponse><result>
        <done>false</done>
        <queryLocator>01gLOC-2000</queryLocator>
        <records xsi:type="sf:sObject"><sf:type>Account</sf:type><sf:Id>001AAA</sf:Id><sf:Name>Acme</sf:Name></records>
        <size>500</size>
    </result></queryResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let (accounts, done, locator): (Vec<TypedAccount>, bool, Option<String>) = client
        .soap()
        .query_typed_page("SELECT Id, Name FROM Account")
        .await
        .must();

    assert!(!done);
    assert_eq!(locator.as_deref(), Some("01gLOC-2000"));
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].name, "Acme");
}

#[tokio::test]
async fn test_query_more_typed_page_returns_records() {
    let (server, client) = setup().await;
    let inner = r#"<queryMoreResponse><result>
        <done>true</done>
        <records xsi:type="sf:sObject"><sf:type>Account</sf:type><sf:Id>001CCC</sf:Id><sf:Name>Gamma</sf:Name></records>
        <size>1</size>
    </result></queryMoreResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let (accounts, done, locator): (Vec<TypedAccount>, bool, Option<String>) = client
        .soap()
        .query_more_typed_page("01gLOC-2000")
        .await
        .must();

    assert!(done);
    assert_eq!(locator, None);
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].name, "Gamma");
}

#[tokio::test]
async fn test_create_typed_null_field_becomes_fields_to_null() {
    let (server, client) = setup().await;
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .and(body_string_contains(
            "<urn:create><urn:sObjects><sf:type>Account</sf:type>",
        ))
        .and(body_string_contains("<sf:Name>Acme</sf:Name>"))
        // A serialized null maps to an explicit fieldsToNull element.
        .and(body_string_contains("<sf:fieldsToNull>Website</sf:fieldsToNull>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(envelope(
            r#"<createResponse><result><id>001AAA</id><success>true</success></result></createResponse>"#,
        )))
        .mount(&server)
        .await;

    let records = vec![TypedAccount {
        id: None,
        name: "Acme".to_string(),
        website: None,
    }];
    let results = client.soap().create_typed("Account", &records).await.must();
    assert!(results[0].success);
    assert_eq!(results[0].id.as_deref(), Some("001AAA"));
}

#[tokio::test]
async fn test_update_typed_sends_id() {
    let (server, client) = setup().await;
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .and(body_string_contains(
            "<urn:update><urn:sObjects><sf:type>Account</sf:type>",
        ))
        .and(body_string_contains("<sf:Id>001AAA</sf:Id>"))
        .and(body_string_contains("<sf:Name>Renamed</sf:Name>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(envelope(
            r#"<updateResponse><result><id>001AAA</id><success>true</success></result></updateResponse>"#,
        )))
        .mount(&server)
        .await;

    let records = vec![TypedAccount {
        id: Some("001AAA".to_string()),
        name: "Renamed".to_string(),
        website: None,
    }];
    let results = client.soap().update_typed("Account", &records).await.must();
    assert!(results[0].success);
}

#[tokio::test]
async fn test_upsert_typed_delegates() {
    let (server, client) = setup().await;
    Mock::given(method("POST"))
        .and(path("/services/Soap/u/67.0"))
        .and(body_string_contains(
            "<urn:externalIDFieldName>Ext__c</urn:externalIDFieldName>",
        ))
        .and(body_string_contains("<sf:Name>Acme</sf:Name>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(envelope(
            r#"<upsertResponse><result><created>true</created><id>001NEW</id><success>true</success></result></upsertResponse>"#,
        )))
        .mount(&server)
        .await;

    let records = vec![TypedAccount {
        id: None,
        name: "Acme".to_string(),
        website: None,
    }];
    let results = client
        .soap()
        .upsert_typed("Account", "Ext__c", &records)
        .await
        .must();
    assert!(results[0].created);
    assert_eq!(results[0].id.as_deref(), Some("001NEW"));
}

#[tokio::test]
async fn test_retrieve_typed_missing_id_yields_none() {
    let (server, client) = setup().await;
    let inner = r#"<retrieveResponse>
        <result xsi:type="sf:sObject"><sf:type>Account</sf:type><sf:Id>001AAA</sf:Id><sf:Name>Acme</sf:Name></result>
        <result xsi:nil="true"/>
    </retrieveResponse>"#;
    mount_soap(&server, 200, &envelope(inner)).await;

    let records: Vec<Option<TypedAccount>> = client
        .soap()
        .retrieve_typed("Account", &["Id", "Name"], &["001AAA", "001MISSING"])
        .await
        .must();

    // Positional: found record then a None for the missing Id.
    assert_eq!(records.len(), 2);
    let found = records[0].as_ref().must();
    assert_eq!(found.id.as_deref(), Some("001AAA"));
    assert_eq!(found.name, "Acme");
    assert!(records[1].is_none());
}

#[tokio::test]
async fn test_create_typed_nested_field_is_error() {
    #[derive(serde::Serialize)]
    struct Nested {
        inner: String,
    }
    #[derive(serde::Serialize)]
    struct BadRecord {
        #[serde(rename = "Name")]
        name: String,
        // A nested object cannot map into the flat Partner field bag.
        #[serde(rename = "Nested")]
        nested: Nested,
    }

    let (_server, client) = setup().await;
    let records = vec![BadRecord {
        name: "Acme".to_string(),
        nested: Nested {
            inner: "x".to_string(),
        },
    }];
    // Serialization fails before any request is dispatched (no mock needed).
    let result = client.soap().create_typed("Account", &records).await;
    let Err(crate::error::ForceError::InvalidInput(msg)) = result else {
        panic!("expected InvalidInput error, got {result:?}");
    };
    assert!(msg.contains("Nested"), "message was: {msg}");
}
