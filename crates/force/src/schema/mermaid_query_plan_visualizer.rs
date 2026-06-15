//! Mermaid Query Plan Visualizer.
//!
//! This module provides a utility to generate Mermaid.js flowchart diagrams
//! from a Query Plan API (`ExplainResponse`) payload.
//!
//! # The Spark
//! We have a `QueryPlanAnalyzer` that warns about slow SOQL queries. What if we
//! could *visualize* the query execution path directly in a PR comment or markdown file?
//! This is an exporter that turns raw JSON plans into a clear, visual flowchart!
//!
//! # Example
//!
//! ```no_run
//! # use force::api::RestOperation;
//! # use force::client::ForceClientBuilder;
//! # use force::schema::generate_mermaid_query_plan;
//! # use force::auth::ClientCredentials;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! let soql = "SELECT Id, Name FROM Account WHERE Name LIKE 'Acme%'";
//! let response = client.rest().explain(soql).await?;
//!
//! let mermaid_diagram = generate_mermaid_query_plan(&response);
//! println!("{}", mermaid_diagram);
//! // graph TD
//! //     Plan0[TableScan] -->|Cost: 1.5| Result
//! # Ok(())
//! # }
//! ```

use crate::types::explain::ExplainResponse;

/// Generates a Mermaid.js flowchart diagram from an `ExplainResponse`.
pub fn generate_mermaid_query_plan(response: &ExplainResponse) -> String {
    let mut mermaid = String::with_capacity(1024);

    mermaid.push_str("graph TD\n");
    mermaid.push_str("    Query[SOQL Query]:::queryNode\n");

    if response.plans.is_empty() {
        mermaid.push_str("    Query --> NoPlans[No Execution Plans Found]\n");
        return mermaid;
    }

    // Find the best plan to highlight it
    let mut best_cost = f64::MAX;
    let mut best_plan_index = 0;

    for (i, plan) in response.plans.iter().enumerate() {
        if plan.relative_cost < best_cost {
            best_cost = plan.relative_cost;
            best_plan_index = i;
        }
    }

    for (i, plan) in response.plans.iter().enumerate() {
        let node_id = format!("Plan{}", i);
        let op_type = &plan.leading_operation_type;

        let mut node_label = format!(
            "{}<br/>Cost: {:.2}<br/>Card: {}",
            op_type, plan.relative_cost, plan.cardinality
        );
        if !plan.fields.is_empty() {
            node_label.push_str(&format!("<br/>Fields: {}", plan.fields.join(", ")));
        }

        mermaid.push_str(&format!("    {}[{}]:::planNode\n", node_id, node_label));
        mermaid.push_str(&format!("    Query -->|Option {}| {}\n", i, node_id));

        if i == best_plan_index {
            mermaid.push_str(&format!(
                "    style {} stroke:#2ecc71,stroke-width:4px\n",
                node_id
            ));
        } else if plan.relative_cost > 1.0 || op_type.eq_ignore_ascii_case("TableScan") {
            mermaid.push_str(&format!(
                "    style {} stroke:#e74c3c,stroke-width:2px,stroke-dasharray: 5 5\n",
                node_id
            ));
        }

        if !plan.notes.is_empty() {
            let notes_id = format!("Notes{}", i);
            let mut notes_label = String::new();
            for (j, note) in plan.notes.iter().enumerate() {
                if j > 0 {
                    notes_label.push_str("<br/>");
                }
                notes_label.push_str(&note.description.replace('"', "'"));
            }
            mermaid.push_str(&format!("    {}[{}]:::noteNode\n", notes_id, notes_label));
            mermaid.push_str(&format!("    {} -.-> {}\n", node_id, notes_id));
        }
    }

    mermaid.push_str(
        "\n    classDef queryNode fill:#3498db,stroke:#2980b9,color:#fff,font-weight:bold\n",
    );
    mermaid.push_str("    classDef planNode fill:#ecf0f1,stroke:#bdc3c7\n");
    mermaid.push_str("    classDef noteNode fill:#fdfad8,stroke:#f1c40f,font-style:italic\n");

    mermaid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::explain::{PlanNote, QueryPlan};

    #[test]
    fn test_generate_mermaid_query_plan() {
        let response = ExplainResponse {
            plans: vec![
                QueryPlan {
                    cardinality: 100,
                    fields: vec!["Name".to_string()],
                    leading_operation_type: "IndexScan".to_string(),
                    notes: vec![PlanNote {
                        description: "Used index".to_string(),
                        fields: vec![],
                        table_enum_or_id: "Account".to_string(),
                    }],
                    relative_cost: 0.5,
                    sobject_cardinality: 1000,
                    sobject_type: "Account".to_string(),
                },
                QueryPlan {
                    cardinality: 1000,
                    fields: vec![],
                    leading_operation_type: "TableScan".to_string(),
                    notes: vec![],
                    relative_cost: 1.5,
                    sobject_cardinality: 1000,
                    sobject_type: "Account".to_string(),
                },
            ],
        };

        let diagram = generate_mermaid_query_plan(&response);
        println!("{}", diagram);

        assert!(diagram.contains("graph TD"));
        assert!(diagram.contains("Query[SOQL Query]:::queryNode"));
        assert!(
            diagram.contains(
                "Plan0[IndexScan<br/>Cost: 0.50<br/>Card: 100<br/>Fields: Name]:::planNode"
            )
        );
        assert!(diagram.contains("style Plan0 stroke:#2ecc71,stroke-width:4px"));
        assert!(diagram.contains("Plan1[TableScan<br/>Cost: 1.50<br/>Card: 1000]:::planNode"));
        assert!(
            diagram.contains("style Plan1 stroke:#e74c3c,stroke-width:2px,stroke-dasharray: 5 5")
        );
        assert!(diagram.contains("Notes0[Used index]:::noteNode"));
    }

    #[test]
    fn test_generate_mermaid_query_plan_empty() {
        let response = ExplainResponse { plans: vec![] };
        let diagram = generate_mermaid_query_plan(&response);
        assert!(diagram.contains("No Execution Plans Found"));
    }
}
