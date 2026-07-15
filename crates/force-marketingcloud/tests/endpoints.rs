//! Happy-path tests for assets, contacts, data extensions, and journeys.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::TestHarness;
use force_marketingcloud::{Asset, AssetType, Contact, InteractionEvent, Row};
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn assets_create_and_get() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("POST"))
        .and(path("/asset/v1/content/assets"))
        .and(body_json(json!({
            "name": "My HTML Email",
            "assetType": { "id": 208, "name": "htmlemail" }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": 42,
            "name": "My HTML Email",
            "assetType": { "id": 208, "name": "htmlemail" }
        })))
        .expect(1)
        .mount(&harness.server)
        .await;

    Mock::given(method("GET"))
        .and(path("/asset/v1/content/assets/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 42,
            "name": "My HTML Email",
            "assetType": { "id": 208, "name": "htmlemail" }
        })))
        .mount(&harness.server)
        .await;

    let asset = Asset::new("My HTML Email", AssetType::new(208, "htmlemail"));
    let created = client.assets().create(&asset).await.unwrap();
    assert_eq!(created.id, Some(42));

    let fetched = client.assets().get(42).await.unwrap();
    assert_eq!(fetched.name, "My HTML Email");
    assert_eq!(fetched.asset_type.id, 208);
}

#[tokio::test]
async fn contacts_create_posts_contact_key() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("POST"))
        .and(path("/contacts/v1/contacts"))
        .and(body_json(json!({
            "contactKey": "jane@example.com",
            "attributeSets": []
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "operationStatus": "OK",
            "contactKey": "jane@example.com"
        })))
        .expect(1)
        .mount(&harness.server)
        .await;

    let contact = Contact::new("jane@example.com");
    let result = client.contacts().create(&contact).await.unwrap();
    assert_eq!(result["operationStatus"], json!("OK"));
}

#[tokio::test]
async fn data_extension_upsert_rows_posts_rowset() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("POST"))
        .and(path("/hub/v1/dataevents/key:MyDE/rowset"))
        .and(body_json(json!([
            {
                "keys": { "SubscriberKey": "c-1" },
                "values": { "FirstName": "John" }
            }
        ])))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&harness.server)
        .await;

    let mut keys = serde_json::Map::new();
    keys.insert("SubscriberKey".to_string(), json!("c-1"));
    let mut values = serde_json::Map::new();
    values.insert("FirstName".to_string(), json!("John"));
    let rows = vec![Row::new(keys, values)];

    client
        .data_extensions()
        .upsert_rows("MyDE", &rows)
        .await
        .unwrap();
}

#[tokio::test]
async fn data_extension_query_rows_returns_items() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("GET"))
        .and(path("/data/v1/customobjectdata/key/MyDE/rowset"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "items": [ { "keys": { "SubscriberKey": "c-1" }, "values": { "FirstName": "John" } } ]
        })))
        .mount(&harness.server)
        .await;

    let result = client.data_extensions().query_rows("MyDE").await.unwrap();
    assert_eq!(result["count"], json!(1));
    assert_eq!(result["items"][0]["values"]["FirstName"], json!("John"));
}

#[tokio::test]
async fn data_extension_insert_rows_async_returns_request_id() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("POST"))
        .and(path("/data/v1/async/dataextensions/key:MyDE/rows"))
        .and(body_json(json!({
            "items": [ { "SubscriberKey": "c-1", "FirstName": "John" } ]
        })))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({
            "requestId": "async-guid",
            "resultMessages": []
        })))
        .expect(1)
        .mount(&harness.server)
        .await;

    let mut keys = serde_json::Map::new();
    keys.insert("SubscriberKey".to_string(), json!("c-1"));
    let mut values = serde_json::Map::new();
    values.insert("FirstName".to_string(), json!("John"));
    let rows = vec![Row::new(keys, values)];

    let response = client
        .data_extensions()
        .insert_rows_async("MyDE", &rows)
        .await
        .unwrap();
    assert_eq!(response.request_id.as_deref(), Some("async-guid"));
}

#[tokio::test]
async fn journeys_list_returns_paged_items() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("GET"))
        .and(path("/interaction/v1/interactions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "page": 1,
            "pageSize": 50,
            "items": [ {
                "id": "guid-1",
                "key": "journey-key",
                "name": "Welcome Journey",
                "version": 1,
                "status": "Published"
            } ]
        })))
        .mount(&harness.server)
        .await;

    let page = client.journeys().list().await.unwrap();
    assert_eq!(page.count, Some(1));
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].name.as_deref(), Some("Welcome Journey"));
}

#[tokio::test]
async fn journeys_fire_event_posts_and_returns_instance_id() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("POST"))
        .and(path("/interaction/v1/events"))
        .and(body_json(json!({
            "ContactKey": "contact-001",
            "EventDefinitionKey": "APIEvent-abc123",
            "Data": { "OrderId": "A-1001" }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "eventInstanceId": "event-guid"
        })))
        .expect(1)
        .mount(&harness.server)
        .await;

    let event = InteractionEvent::new("contact-001", "APIEvent-abc123")
        .with_data(json!({ "OrderId": "A-1001" }));
    let response = client.journeys().fire_event(&event).await.unwrap();
    assert_eq!(response.event_instance_id.as_deref(), Some("event-guid"));
}

#[tokio::test]
async fn raw_escape_hatch_get_works() {
    let harness = TestHarness::start().await;
    let client = harness.client();

    Mock::given(method("GET"))
        .and(path("/some/custom/endpoint"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(&harness.server)
        .await;

    let value = client.raw_get("some/custom/endpoint", None).await.unwrap();
    assert_eq!(value["ok"], json!(true));
}
