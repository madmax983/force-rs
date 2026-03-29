import re

def inject_test(filepath, new_code):
    with open(filepath, 'r') as f:
        content = f.read()

    # Find the last closing brace of the file
    last_brace_idx = content.rfind('}')

    if last_brace_idx != -1:
        updated_content = content[:last_brace_idx] + new_code + "\n}\n"
        with open(filepath, 'w') as f:
            f.write(updated_content)
        print(f"Successfully injected test into {filepath}")
    else:
        print(f"Failed to find closing brace in {filepath}")

# Token Manager
token_test = """
    #[tokio::test]
    async fn test_token_manager_latest_token_or_logic() {
        let auth = MockAuthenticator::new();
        let manager = TokenManager::new(auth);

        let old_token = AccessToken::new(
            "old".to_string(),
            "url".to_string(),
            Some(Utc::now() - Duration::hours(1))
        );

        let new_token = AccessToken::new(
            "new".to_string(),
            "url".to_string(),
            Some(Utc::now() + Duration::hours(1))
        );

        {
            let mut state = manager.state.write().await;
            state.token = Some(StdArc::new(new_token.clone()));
        }

        // Fallback is older, current should be returned
        let result1 = manager.latest_token_or(StdArc::new(old_token.clone())).await;
        assert_eq!(result1.as_str(), "new");

        {
            let mut state = manager.state.write().await;
            state.token = Some(StdArc::new(old_token.clone()));
        }

        // Fallback is newer, fallback should be returned
        let result2 = manager.latest_token_or(StdArc::new(new_token.clone())).await;
        assert_eq!(result2.as_str(), "new");
    }
"""
inject_test('crates/force/src/auth/token_manager.rs', token_test)

# Builder
builder_test = """
    #[tokio::test]
    #[cfg(feature = "data_cloud")]
    async fn test_builder_data_cloud_config() {
        let auth = MockAuthenticator::new("mock", "url");
        let dc_config = crate::auth::DataCloudConfig {
            api_version: Some("v99.0".to_string()),
            ..Default::default()
        };
        let client = ForceClientBuilder::new()
            .authenticate(auth)
            .with_data_cloud(dc_config)
            .build()
            .await
            .must();
        assert_eq!(client.dc_session.unwrap().config.api_version, "v99.0");
    }
"""
inject_test('crates/force/src/client/builder.rs', builder_test)

# Mass Op
mass_op_test = """
    #[tokio::test]
    async fn test_mass_update_invalid_input() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;
        let query = SoqlQueryBuilder::new().select(&["Id"]).from("Account");
        let op = SoqlMassOp::new(&client, query);

        let updates = json!("not an object");
        let result = op.update_all(updates).await;

        let Err(err) = result else { panic!("Expected an error for invalid input") };
        assert!(matches!(err, ForceError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_mass_op_asserts_non_default_stats() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 1,
                "done": true,
                "records": [ { "attributes": { "type": "Account" }, "Id": "001A" } ]
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
        let stats = op.delete_all().await.must();

        assert_ne!(stats.records_processed, 0);
        assert_ne!(stats.ops_succeeded, 0);
    }
"""
inject_test('crates/force/src/experimental/soql_mass_op.rs', mass_op_test)

# Query Plan
query_plan_test = """
    #[test]
    fn test_analyze_query_plan_initialization_fields() {
        let response = ExplainResponse {
            plans: vec![
                create_plan("IndexScan", 5.0, 10, 10000, vec![]),
                create_plan("TableScan", 10.0, 10, 10000, vec![]),
            ],
        };
        let insights = analyze_query_plan(&response);
        assert_eq!(insights.evaluated_plans, 2);
        assert!((insights.lowest_cost - 5.0).abs() < f64::EPSILON);
    }
"""
inject_test('crates/force/src/experimental/query_plan_analyzer.rs', query_plan_test)
