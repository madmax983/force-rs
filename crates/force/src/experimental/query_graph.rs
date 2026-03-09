//! SOQL Query Graph Visualizer.
//!
//! This module provides a utility to parse SOQL query strings and generate
//! Mermaid.js Entity Relationship (ER) diagrams of the object relationships
//! involved in the query.
//!
//! # Example
//!
//! ```
//! # use force::experimental::query_graph::QueryGraph;
//! let query = "SELECT Id, Name, Account.Name, (SELECT Id, FirstName FROM Contacts) FROM Opportunity";
//! let graph = QueryGraph::new(query);
//!
//! println!("{}", graph.to_mermaid());
//! ```

/// A utility to generate Mermaid.js ER diagrams from SOQL queries.
#[derive(Debug, Clone)]
pub struct QueryGraph {
    soql: String,
}

use std::collections::HashSet;

impl QueryGraph {
    /// Creates a new `QueryGraph` instance from a SOQL query string.
    #[must_use]
    pub fn new(soql: impl Into<String>) -> Self {
        Self { soql: soql.into() }
    }

    /// Generates a Mermaid.js ER diagram representing the relationships
    /// found in the SOQL query.
    #[must_use]
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("erDiagram\n");

        let tokens = Self::tokenize(&self.soql);

        let mut parent_rels = HashSet::new();
        let mut child_rels = HashSet::new();
        let mut main_object = String::new();

        let mut in_select = false;
        let mut in_subquery = 0;
        let mut expecting_main_object = false;

        for token in &tokens {
            let token_upper = token.to_uppercase();

            if token_upper == "SELECT" {
                if in_subquery == 0 {
                    in_select = true;
                }
            } else if token_upper == "FROM" {
                if in_subquery == 0 {
                    in_select = false;
                    expecting_main_object = true;
                }
            } else if token == "(" {
                in_subquery += 1;
            } else if token == ")" {
                in_subquery -= 1;
            } else if expecting_main_object {
                main_object.clone_from(token);
                expecting_main_object = false;
            } else if in_select && in_subquery == 0 && token.contains('.') {
                if token.split('.').count() > 1 {
                    parent_rels.insert(token.clone());
                }
            }
        }

        // Find subqueries via FROM within parens
        for i in 0..tokens.len() {
            if tokens[i].to_uppercase() == "FROM" {
                // If it's inside parens, the next token is likely the child relationship
                // Simple heuristic: look backwards to see if we are in a subquery
                let mut in_parens = 0;
                for j in (0..i).rev() {
                    if tokens[j] == ")" {
                        in_parens -= 1;
                    } else if tokens[j] == "(" {
                        in_parens += 1;
                    }
                }

                if in_parens > 0 && i + 1 < tokens.len() {
                    child_rels.insert(tokens[i + 1].clone());
                }
            }
        }

        if parent_rels.is_empty() && child_rels.is_empty() {
            if !main_object.is_empty() {
                mermaid.push_str(&format!("  {}\n", main_object));
            }
        } else {
            // Child relationships (e.g., Contacts, Opportunities)
            let mut sorted_children: Vec<_> = child_rels.into_iter().collect();
            sorted_children.sort();
            for child in sorted_children {
                mermaid.push_str(&format!(
                    "  {} ||--o{{ {} : \"({1})\"\n",
                    main_object, child
                ));
            }

            // Parent relationships via dot notation (e.g., Account.Name, Account.Owner.Name)
            let mut sorted_parents: Vec<_> = parent_rels.into_iter().collect();
            sorted_parents.sort();
            for parent_rel in sorted_parents {
                let parts: Vec<&str> = parent_rel.split('.').collect();
                let mut current_obj = main_object.clone();

                for next_obj in parts.iter().take(parts.len() - 1) {
                    mermaid.push_str(&format!(
                        "  {} }}o--|| {} : \"{}\"\n",
                        current_obj, next_obj, parent_rel
                    ));
                    current_obj = (*next_obj).to_string();
                }
            }
        }

        mermaid
    }

    fn tokenize(soql: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current_token = String::new();

        for c in soql.chars() {
            if c.is_whitespace() || c == ',' {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
            } else if c == '(' || c == ')' {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
                tokens.push(c.to_string());
            } else {
                current_token.push(c);
            }
        }

        if !current_token.is_empty() {
            tokens.push(current_token);
        }

        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_query() {
        let query = "SELECT Id FROM Account";
        let graph = QueryGraph::new(query);
        let mermaid = graph.to_mermaid();

        // A basic query without relationships just shows the main object
        assert_eq!(mermaid.trim(), "erDiagram\n  Account");
    }

    #[test]
    fn test_parent_relationship() {
        let query = "SELECT Id, Account.Name FROM Contact";
        let graph = QueryGraph::new(query);
        let mermaid = graph.to_mermaid();

        assert!(mermaid.contains("Contact }o--|| Account : \"Account.Name\""));
    }

    #[test]
    fn test_child_relationship() {
        let query = "SELECT Id, (SELECT Id FROM Contacts) FROM Account";
        let graph = QueryGraph::new(query);
        let mermaid = graph.to_mermaid();

        assert!(mermaid.contains("Account ||--o{ Contacts : \"(Contacts)\""));
    }

    #[test]
    fn test_complex_query() {
        let query = "SELECT Id, Account.Owner.Name, (SELECT Id FROM Opportunities), (SELECT Id FROM Cases) FROM Contact";
        let graph = QueryGraph::new(query);
        let mermaid = graph.to_mermaid();

        assert!(mermaid.contains("Contact }o--|| Account : \"Account.Owner.Name\""));
        assert!(mermaid.contains("Account }o--|| Owner : \"Account.Owner.Name\""));
        assert!(mermaid.contains("Contact ||--o{ Opportunities : \"(Opportunities)\""));
        assert!(mermaid.contains("Contact ||--o{ Cases : \"(Cases)\""));
    }
}
