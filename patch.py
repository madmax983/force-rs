import re

with open("crates/force/src/api/rest/crud.rs", "r") as f:
    content = f.read()

tests = """
    #[tokio::test]
    async fn test_upsert_idempotent_returns_other_success_code() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-002",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "001xx000003DHP0AAO",
                "success": true,
                "created": false,
                "errors": []
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let response = rest
            .upsert_idempotent(
                "Account",
                "ExternalId__c",
                "ACME-002",
                &json!({"Name": "Acme Corp 2"}),
            )
            .await
            .must();

        assert!(response.is_success());
        assert!(!response.is_created());
        assert_eq!(response.id.as_str(), "001xx000003DHP0AAO");
    }

    #[tokio::test]
    async fn test_upsert_idempotent_returns_204_not_implemented() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-001",
            ))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let result = rest
            .upsert_idempotent(
                "Account",
                "ExternalId__c",
                "ACME-001",
                &json!({"Name": "Acme Corp"}),
            )
            .await;

        assert!(matches!(
            result,
            Err(crate::error::ForceError::NotImplemented(_))
        ));
    }

    #[tokio::test]
    async fn test_upsert_idempotent_returns_400_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/ExternalId__c/ACME-001",
            ))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
                "message": "Provided external ID field does not exist or is not accessible",
                "statusCode": "INVALID_FIELD"
            }])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let rest = client.rest();
        let result = rest
            .upsert_idempotent(
                "Account",
                "ExternalId__c",
                "ACME-001",
                &json!({"Name": "Acme Corp"}),
            )
            .await;

        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("Provided external ID field does not exist or is not accessible")
        );
    }
"""

content = re.sub(r"}\s*$", tests + "\n}\n", content)

with open("crates/force/src/api/rest/crud.rs", "w") as f:
    f.write(content)
