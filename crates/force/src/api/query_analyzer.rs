//! Query Health Analyzer.
//!
//! This module provides `QueryHealthAnalyzer`, a utility that combines the diagnostic
//! power of the Salesforce Query Plan API with heuristic evaluations to score the
//! health and efficiency of a SOQL query.
//!
//! # The Spark
//! We have the `explain()` endpoint which returns raw execution plans. But developers
//! often don't know how to interpret "relativeCost" or "leadingOperationType".
//! What if we built an analyzer that takes the raw `ExplainResponse` and distills
//! it into an actionable "Health Score" and clear warnings?

use crate::types::explain::ExplainResponse;

/// The overall health evaluation of a SOQL query.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryHealth {
    /// Score from 0 to 100 (100 is optimal).
    pub score: u8,
    /// Indicates if the query is a full table scan.
    pub is_table_scan: bool,
    /// Warns if the query returns a large percentage of the table.
    pub unselective_warning: bool,
    /// Human-readable diagnostic messages.
    pub diagnostics: Vec<String>,
}

/// Utility for analyzing SOQL query execution plans.
#[derive(Debug, Default)]
pub struct QueryHealthAnalyzer;

impl QueryHealthAnalyzer {
    /// Creates a new `QueryHealthAnalyzer`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Analyzes an `ExplainResponse` to determine the query's health.
    #[must_use]
    pub fn analyze(&self, explain: &ExplainResponse) -> QueryHealth {
        let mut health = QueryHealth {
            score: 100,
            is_table_scan: false,
            unselective_warning: false,
            diagnostics: Vec::new(),
        };

        if explain.plans.is_empty() {
            health.score = 0;
            health
                .diagnostics
                .push("No execution plans found.".to_string());
            return health;
        }

        // Sort plans to find the one with the lowest relative cost
        let mut sorted_plans = explain.plans.clone();
        sorted_plans.sort_by(|a, b| {
            a.relative_cost
                .partial_cmp(&b.relative_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let best_plan = &sorted_plans[0];

        // 1. Table Scan Penalty
        if best_plan
            .leading_operation_type
            .eq_ignore_ascii_case("TableScan")
        {
            health.is_table_scan = true;
            health.score = health.score.saturating_sub(40);
            health
                .diagnostics
                .push("Full Table Scan detected. The query does not use an index.".to_string());
        }

        // 2. Selectivity Penalty
        if best_plan.sobject_cardinality > 0 {
            let selectivity =
                (best_plan.cardinality as f64) / (best_plan.sobject_cardinality as f64);
            if selectivity > 0.1 {
                // More than 10% of the table
                health.unselective_warning = true;
                health.score = health.score.saturating_sub(30);
                health.diagnostics.push(format!("Query is unselective, returning {:.1}% of the table records. Consider refining filters.", selectivity * 100.0));
            }
        }

        // 3. Cost Penalty
        if best_plan.relative_cost > 1.0 {
            health.score = health.score.saturating_sub(20);
            health.diagnostics.push(format!(
                "High relative cost: {:.2}. Performance may be degraded.",
                best_plan.relative_cost
            ));
        }

        for note in &best_plan.notes {
            health
                .diagnostics
                .push(format!("Salesforce Note: {}", note.description));
        }

        health
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::explain::{PlanNote, QueryPlan};

    #[test]
    fn test_optimal_plan() {
        let analyzer = QueryHealthAnalyzer::new();
        let plan = QueryPlan {
            cardinality: 10,
            fields: vec!["Id".to_string()],
            leading_operation_type: "IndexScan".to_string(),
            notes: vec![],
            relative_cost: 0.1,
            sobject_cardinality: 10000,
            sobject_type: "Account".to_string(),
        };
        let explain = ExplainResponse { plans: vec![plan] };

        let health = analyzer.analyze(&explain);
        assert_eq!(health.score, 100);
        assert!(!health.is_table_scan);
        assert!(!health.unselective_warning);
        assert!(health.diagnostics.is_empty());
    }

    #[test]
    fn test_table_scan_plan() {
        let analyzer = QueryHealthAnalyzer::new();
        let plan = QueryPlan {
            cardinality: 10,
            fields: vec![],
            leading_operation_type: "TableScan".to_string(),
            notes: vec![],
            relative_cost: 0.5,
            sobject_cardinality: 10000,
            sobject_type: "Account".to_string(),
        };
        let explain = ExplainResponse { plans: vec![plan] };

        let health = analyzer.analyze(&explain);
        assert_eq!(health.score, 60); // 100 - 40
        assert!(health.is_table_scan);
        assert!(!health.unselective_warning);
        assert_eq!(health.diagnostics.len(), 1);
    }

    #[test]
    fn test_unselective_plan() {
        let analyzer = QueryHealthAnalyzer::new();
        let plan = QueryPlan {
            cardinality: 5000,
            fields: vec![],
            leading_operation_type: "IndexScan".to_string(),
            notes: vec![],
            relative_cost: 0.5,
            sobject_cardinality: 10000,
            sobject_type: "Account".to_string(),
        };
        let explain = ExplainResponse { plans: vec![plan] };

        let health = analyzer.analyze(&explain);
        assert_eq!(health.score, 70); // 100 - 30
        assert!(!health.is_table_scan);
        assert!(health.unselective_warning);
        assert_eq!(health.diagnostics.len(), 1);
    }

    #[test]
    fn test_terrible_plan() {
        let analyzer = QueryHealthAnalyzer::new();
        let plan = QueryPlan {
            cardinality: 5000,
            fields: vec![],
            leading_operation_type: "TableScan".to_string(),
            notes: vec![PlanNote {
                description: "Not indexed".to_string(),
                fields: vec!["Name".to_string()],
                table_enum_or_id: "Account".to_string(),
            }],
            relative_cost: 1.5,
            sobject_cardinality: 10000,
            sobject_type: "Account".to_string(),
        };
        let explain = ExplainResponse { plans: vec![plan] };

        let health = analyzer.analyze(&explain);
        // 100 - 40 (TableScan) - 30 (Unselective) - 20 (High Cost) = 10
        assert_eq!(health.score, 10);
        assert!(health.is_table_scan);
        assert!(health.unselective_warning);
        assert_eq!(health.diagnostics.len(), 4);
    }

    #[test]
    fn test_empty_plans() {
        let analyzer = QueryHealthAnalyzer::new();
        let explain = ExplainResponse { plans: vec![] };

        let health = analyzer.analyze(&explain);
        assert_eq!(health.score, 0);
        assert_eq!(health.diagnostics.len(), 1);
    }
}
