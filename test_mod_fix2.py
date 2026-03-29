import re

filepath = 'crates/force/src/experimental/soql_mass_op.rs'
with open(filepath, 'r') as f:
    content = f.read()

# We mistakenly placed the new test OUTSIDE the `mod tests` block because we appended to the end of the file.
# We will read it, remove the badly appended test, and correctly place it inside `mod tests`.

content = content.replace("""#[tokio::test]
async fn test_mass_update_asserts_non_default_stats() {
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let auth = MockAuthenticator::new("test_token", &mock_server.uri());
    let client = builder().authenticate(auth).build().await.must();

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 1,
            "done": true,
            "records": [ { "attributes": { "type": "Account", "url": "/services/data/v60.0/sobjects/Account/001000000000001AAA" }, "Id": "001000000000001AAA" } ]
        })))
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/composite/batch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hasErrors": false,
            "results": [ { "statusCode": 204, "result": null } ]
        })))
        .mount(&mock_server)
        .await;

    let query = crate::api::soql::SoqlQueryBuilder::new().select(&["Id"]).from("Account");
    let op = crate::experimental::SoqlMassOp::new(&client, query);
    let updates = json!({"Status": "Closed"});
    let stats = op.update_all(updates).await.must();

    assert_ne!(stats.records_processed, 0);
    assert_ne!(stats.ops_succeeded, 0);
}
""", "")

last_brace_idx = content.rfind('}')
if last_brace_idx != -1:
    new_test = """
    #[tokio::test]
    async fn test_mass_update_asserts_non_default_stats() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 1,
                "done": true,
                "records": [ { "attributes": { "type": "Account", "url": "/services/data/v60.0/sobjects/Account/001000000000001AAA" }, "Id": "001000000000001AAA" } ]
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/batch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "hasErrors": false,
                "results": [ { "statusCode": 204, "result": null } ]
            })))
            .mount(&mock_server)
            .await;

        let query = SoqlQueryBuilder::new().select(&["Id"]).from("Account");
        let op = SoqlMassOp::new(&client, query);
        let updates = json!({"Status": "Closed"});
        let stats = op.update_all(updates).await.must();

        assert_ne!(stats.records_processed, Default::default());
        assert_ne!(stats.ops_succeeded, Default::default());
    }
"""
    updated_content = content[:last_brace_idx] + new_test + "\n}\n"
    with open(filepath, 'w') as f:
        f.write(updated_content)
