//! GraphQL API Example
//!
//! Demonstrates using the Salesforce GraphQL API for queries with typed
//! results, variables, and error handling.
//!
//! The GraphQL API provides a unified query interface where you can request
//! specific fields, traverse relationships, and query multiple objects in a
//! single request — all through one POST endpoint.
//!
//! # Setup
//!
//! Set the following environment variables:
//! - `SF_CLIENT_ID`      - OAuth client ID (Connected App Consumer Key)
//! - `SF_CLIENT_SECRET`  - OAuth client secret (Connected App Consumer Secret)
//!
//! # Run
//!
//! ```bash
//! SF_CLIENT_ID=xxx SF_CLIENT_SECRET=yyy cargo run --example graphql --features graphql
//! ```

#[cfg(feature = "graphql")]
mod example {
    use anyhow::{Context, Result};
    use force::api::graphql::{GraphqlRequest, GraphqlResponse};
    use force::auth::ClientCredentials;
    use force::client::ForceClientBuilder;
    use serde::Deserialize;
    use serde_json::{Value, json};

    pub async fn run() -> Result<()> {
        let client_id = std::env::var("SF_CLIENT_ID").context("SF_CLIENT_ID not set")?;
        let client_secret =
            std::env::var("SF_CLIENT_SECRET").context("SF_CLIENT_SECRET not set")?;

        println!("Authenticating with Salesforce...");
        let auth = ClientCredentials::new_production(client_id, client_secret);
        let client = ForceClientBuilder::new()
            .authenticate(auth)
            .build()
            .await
            .context("Failed to build client")?;

        let gql = client.graphql();
        println!("Authentication successful\n");

        // ── 1. Simple Raw Query ─────────────────────────────────────────
        println!("=== Simple Query (raw Value) ===");
        let data = gql
            .query_raw(
                r#"{
                    uiapi {
                        query {
                            Account(first: 5) {
                                edges {
                                    node {
                                        Id
                                        Name { value }
                                        Industry { value displayValue }
                                    }
                                }
                                totalCount
                            }
                        }
                    }
                }"#,
                None,
            )
            .await?;

        let accounts = &data["uiapi"]["query"]["Account"];
        println!("  Total accounts: {}", accounts["totalCount"]);
        if let Some(edges) = accounts["edges"].as_array() {
            for edge in edges {
                let node = &edge["node"];
                println!(
                    "  - {} ({})",
                    node["Name"]["value"].as_str().unwrap_or("?"),
                    node["Industry"]["displayValue"].as_str().unwrap_or("N/A"),
                );
            }
        }
        println!();

        // ── 2. Query with Variables ──────────────────────────────────────
        println!("=== Query with Variables ===");
        let req = GraphqlRequest::new(
            r#"query GetAccounts($limit: Int) {
                uiapi {
                    query {
                        Account(first: $limit) {
                            edges {
                                node {
                                    Id
                                    Name { value }
                                }
                                cursor
                            }
                            pageInfo {
                                hasNextPage
                            }
                        }
                    }
                }
            }"#,
        )
        .with_variables(json!({"limit": 3}))
        .with_operation_name("GetAccounts");

        let data: Value = gql.query(&req).await?;
        let page_info = &data["uiapi"]["query"]["Account"]["pageInfo"];
        println!(
            "  Has next page: {}",
            page_info["hasNextPage"].as_bool().unwrap_or(false)
        );
        println!();

        // ── 3. Typed Deserialization ─────────────────────────────────────
        println!("=== Typed Query ===");

        #[derive(Debug, Deserialize)]
        struct UiApi {
            uiapi: UiApiQuery,
        }

        #[derive(Debug, Deserialize)]
        struct UiApiQuery {
            query: AccountQuery,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct AccountQuery {
            account: Connection,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Connection {
            edges: Vec<Edge>,
            total_count: u32,
        }

        #[derive(Debug, Deserialize)]
        struct Edge {
            node: AccountNode,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct AccountNode {
            id: String,
            name: FieldValue,
        }

        #[derive(Debug, Deserialize)]
        struct FieldValue {
            value: Option<String>,
        }

        let req = GraphqlRequest::new(
            r#"{
                uiapi {
                    query {
                        Account(first: 3) {
                            edges {
                                node {
                                    Id
                                    Name { value }
                                }
                            }
                            totalCount
                        }
                    }
                }
            }"#,
        );

        let typed: UiApi = gql.query(&req).await?;
        println!("  Total: {}", typed.uiapi.query.account.total_count);
        for edge in &typed.uiapi.query.account.edges {
            println!(
                "  - {} ({})",
                edge.node.name.value.as_deref().unwrap_or("?"),
                edge.node.id,
            );
        }
        println!();

        // ── 4. Partial Success Handling ──────────────────────────────────
        println!("=== Partial Success (query_with_errors) ===");
        let req = GraphqlRequest::new(
            r#"{
                uiapi {
                    query {
                        Account(first: 1) {
                            edges {
                                node {
                                    Id
                                    Name { value }
                                }
                            }
                        }
                    }
                }
            }"#,
        );

        let envelope: GraphqlResponse<Value> = gql.query_with_errors(&req).await?;
        if envelope.has_errors() {
            println!("  Warnings/errors in response:");
            if let Some(errors) = &envelope.errors {
                for err in errors {
                    println!("    - {}", err.message);
                }
            }
        } else {
            println!("  No errors in response");
        }
        if envelope.data.is_some() {
            println!("  Data received successfully");
        }

        // ── 5. Error Handling ────────────────────────────────────────────
        println!("\n=== Error Handling ===");
        let bad_req = GraphqlRequest::new("{ invalid { query } }");
        match gql.query::<Value>(&bad_req).await {
            Ok(_) => println!("  Query succeeded (unexpected)"),
            Err(e) => println!("  Expected error: {e}"),
        }

        println!("\nGraphQL API example complete.");
        Ok(())
    }
}

#[cfg(feature = "graphql")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    example::run().await
}

#[cfg(not(feature = "graphql"))]
fn main() {
    eprintln!("This example requires the 'graphql' feature.");
    eprintln!("Run with: cargo run --example graphql --features graphql");
}
