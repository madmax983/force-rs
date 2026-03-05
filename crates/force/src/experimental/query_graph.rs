//! Salesforce SOQL Query Graph Visualizer.
//!
//! This module provides a utility to parse a SOQL query string and generate
//! Mermaid.js Entity Relationship (ER) diagrams representing the object
//! relationships traversed in the query.
//!
//! # Example
//!
//! ```
//! use force::experimental::query_graph::QueryGraph;
//!
//! let query = "SELECT Id, Name, Account.Name, Owner.FirstName FROM Contact";
//! let graph = QueryGraph::from_soql(query);
//!
//! println!("{}", graph.to_mermaid());
//! // erDiagram
//! //   Account ||--o{ Contact : "Account"
//! //   Owner ||--o{ Contact : "Owner"
//! ```

use std::collections::HashSet;

/// A graph representing the relationships in a SOQL query.
#[derive(Debug, Default, Clone)]
pub struct QueryGraph {
    base_object: String,
    relationships: HashSet<(String, String)>, // (Target, FieldName)
}

impl QueryGraph {
    /// Parses a basic SOQL query string and extracts object relationships.
    ///
    /// This uses a simplistic string-based heuristic rather than a full SOQL parser.
    /// It extracts the `FROM` object as the base, and looks for dot-notation in the `SELECT`
    /// clause to infer lookup relationships (e.g., `Account.Name` implies a relationship
    /// to `Account` via the `Account` lookup field on the base object).
    #[must_use]
    pub fn from_soql(soql: &str) -> Self {
        let mut graph = Self::default();

        // Very basic parsing
        // We use a case-insensitive search that preserves byte indices of the original string.
        let select_idx = soql
            .char_indices()
            .find(|(i, _)| {
                soql[*i..].starts_with("SELECT ")
                    || soql[*i..].starts_with("select ")
                    || soql[*i..].starts_with("Select ")
            })
            .map(|(i, _)| i);

        let from_idx = soql
            .char_indices()
            .find(|(i, _)| {
                soql[*i..].starts_with(" FROM ")
                    || soql[*i..].starts_with(" from ")
                    || soql[*i..].starts_with(" From ")
            })
            .map(|(i, _)| i);

        if let (Some(s_idx), Some(f_idx)) = (select_idx, from_idx) {
            if s_idx < f_idx {
                // Extract FROM object
                let after_from = &soql[f_idx + 6..];
                // Take the first word after FROM
                let base_obj = after_from
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_matches(&[',', ';', ')'][..]);
                graph.base_object = base_obj.to_string();

                // Extract SELECT fields
                let select_clause = &soql[s_idx + 7..f_idx];
                for field in select_clause.split(',') {
                    let field = field.trim();
                    if let Some(dot_idx) = field.find('.') {
                        // Found a relationship e.g., "Account.Name" or "Owner.Id"
                        // The part before the dot is typically the relationship name, which often matches the target object
                        // (e.g., standard lookups like Account.Name where relationship is Account and target is Account).
                        // For custom relationships (e.g., Custom__r.Name), we'll just use the relationship name.
                        let rel_name = &field[..dot_idx];

                        // Heuristic: if it ends in __r, the target might be __c, but without describe metadata
                        // we'll just keep the __r or relationship name to represent the connection.
                        graph
                            .relationships
                            .insert((rel_name.to_string(), rel_name.to_string()));
                    }
                }
            }
        }

        graph
    }

    /// Generates a Mermaid.js ER diagram from the parsed query relationships.
    #[must_use]
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("erDiagram\n");

        if self.base_object.is_empty() {
            return mermaid;
        }

        // Add base object entity just to ensure it appears even if no relationships
        mermaid.push_str(&format!("    {} {{\n    }}\n\n", self.base_object));

        // Sort relationships for deterministic output
        let mut rels: Vec<_> = self.relationships.iter().collect();
        rels.sort();

        // Target ||--o{ BaseObject : RelationshipName
        for (target, rel_name) in rels {
            mermaid.push_str(&format!(
                "    {} ||--o{{ {} : \"{}\"\n",
                target, self.base_object, rel_name
            ));
        }

        mermaid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_soql_parsing() {
        let query = "SELECT Id, Name, Account.Name FROM Contact";
        let graph = QueryGraph::from_soql(query);

        assert_eq!(graph.base_object, "Contact");
        assert_eq!(graph.relationships.len(), 1);
        assert!(
            graph
                .relationships
                .contains(&("Account".to_string(), "Account".to_string()))
        );
    }

    #[test]
    fn test_multiple_relationships() {
        let query = "SELECT Id, Account.Name, Owner.Id, Custom__r.Field FROM Opportunity";
        let graph = QueryGraph::from_soql(query);

        assert_eq!(graph.base_object, "Opportunity");
        assert_eq!(graph.relationships.len(), 3);
        assert!(
            graph
                .relationships
                .contains(&("Account".to_string(), "Account".to_string()))
        );
        assert!(
            graph
                .relationships
                .contains(&("Owner".to_string(), "Owner".to_string()))
        );
        assert!(
            graph
                .relationships
                .contains(&("Custom__r".to_string(), "Custom__r".to_string()))
        );
    }

    #[test]
    fn test_no_relationships() {
        let query = "SELECT Id, Name FROM Account";
        let graph = QueryGraph::from_soql(query);

        assert_eq!(graph.base_object, "Account");
        assert!(graph.relationships.is_empty());
    }

    #[test]
    fn test_mermaid_generation() {
        let query = "SELECT Id, Account.Name FROM Contact";
        let graph = QueryGraph::from_soql(query);
        let mermaid = graph.to_mermaid();

        assert!(mermaid.contains("erDiagram"));
        assert!(mermaid.contains("Contact {"));
        assert!(mermaid.contains("Account ||--o{ Contact : \"Account\""));
    }
}
