//! SOQL Query Graph Visualizer.
//!
//! This module provides a utility to parse a SOQL query string and generate
//! a Mermaid.js Entity Relationship (ER) diagram representing the objects and
//! relationships involved in the query.
//!
//! # Example
//!
//! ```no_run
//! # use force::experimental::query_graph::QueryGraph;
//! # fn main() {
//! let soql = "SELECT Id, Name, Account.Name, (SELECT Contact.LastName FROM Contacts) FROM Opportunity";
//! let graph = QueryGraph::parse(soql);
//! println!("{}", graph.to_mermaid());
//! // erDiagram
//! //   Opportunity ||--o| Account : "Account"
//! //   Opportunity ||--o{ Contact : "Contacts"
//! # }
//! ```

use std::collections::HashSet;

/// Represents a parsed SOQL query's structure for visualization.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryGraph {
    /// The base object being queried (the `FROM` clause).
    pub base_object: String,
    /// Lookup relationships (e.g., `Account.Name` -> `Account`).
    pub lookups: HashSet<String>,
    /// Child relationships (subqueries, e.g., `(SELECT ... FROM Contacts)` -> `Contacts`).
    pub child_relationships: HashSet<String>,
}

impl QueryGraph {
    /// Parses a SOQL query string into a `QueryGraph`.
    ///
    /// This is a lightweight parser designed specifically for extracting
    /// object relationships for visualization. It is not a full SOQL AST parser.
    #[must_use]
    pub fn parse(query: &str) -> Self {
        let mut base_object = String::new();
        let mut lookups = HashSet::new();
        let mut child_relationships = HashSet::new();

        // 1. Find the base object (the word after FROM, outside of any subqueries)
        // We'll do a simple scan for FROM ignoring what's inside parentheses.
        let mut depth = 0;
        let mut tokens = Vec::new();
        let mut current_token = String::new();

        for c in query.chars() {
            match c {
                '(' => {
                    if !current_token.trim().is_empty() {
                        tokens.push((depth, current_token.clone()));
                        current_token.clear();
                    }
                    depth += 1;
                    tokens.push((depth, "(".to_string()));
                }
                ')' => {
                    if !current_token.trim().is_empty() {
                        tokens.push((depth, current_token.clone()));
                        current_token.clear();
                    }
                    tokens.push((depth, ")".to_string()));
                    depth -= 1;
                }
                ' ' | ',' | '\n' | '\t' => {
                    if !current_token.trim().is_empty() {
                        tokens.push((depth, current_token.clone()));
                        current_token.clear();
                    }
                }
                _ => {
                    current_token.push(c);
                }
            }
        }
        if !current_token.trim().is_empty() {
            tokens.push((depth, current_token.clone()));
        }

        let mut in_select = false;
        let mut next_is_from = false;

        for (d, token) in &tokens {
            let upper = token.to_uppercase();
            if *d == 0 {
                if upper == "SELECT" {
                    in_select = true;
                } else if upper == "FROM" {
                    in_select = false;
                    next_is_from = true;
                } else if next_is_from {
                    base_object.clone_from(token);
                    next_is_from = false;
                } else if in_select {
                    // Extract lookups like Account.Name
                    if let Some(idx) = token.find('.') {
                        lookups.insert(token[..idx].to_string());
                    }
                }
            } else if *d == 1 && upper == "FROM" {
                // Look ahead for the next token at depth 1 which is the child object
            }
        }

        // Second pass to get child relationships
        for i in 0..tokens.len() {
            let (d, token) = &tokens[i];
            if *d == 1 && token.to_uppercase() == "FROM" {
                if i + 1 < tokens.len() {
                    let (next_d, next_token) = &tokens[i + 1];
                    if *next_d == 1 {
                        child_relationships.insert(next_token.clone());
                    }
                }
            }
        }

        Self {
            base_object,
            lookups,
            child_relationships,
        }
    }

    /// Generates a Mermaid.js Entity Relationship diagram.
    #[must_use]
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("erDiagram\n");

        if self.base_object.is_empty()
            && self.lookups.is_empty()
            && self.child_relationships.is_empty()
        {
            return mermaid;
        }

        // Just emit the base object if no relationships exist
        if self.lookups.is_empty() && self.child_relationships.is_empty() {
            mermaid.push_str(&format!("  {}\n", self.base_object));
        }

        // Lookups: Base ||--o| Lookup : "LookupName"
        let mut sorted_lookups: Vec<_> = self.lookups.iter().collect();
        sorted_lookups.sort();
        for lookup in sorted_lookups {
            mermaid.push_str(&format!(
                "  {} ||--o| {} : \"{}\"\n",
                self.base_object, lookup, lookup
            ));
        }

        // Children: Base ||--o{ Child : "ChildName"
        let mut sorted_children: Vec<_> = self.child_relationships.iter().collect();
        sorted_children.sort();
        for child in sorted_children {
            mermaid.push_str(&format!(
                "  {} ||--o{{ {} : \"{}\"\n",
                self.base_object, child, child
            ));
        }

        mermaid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_query() {
        let q = "SELECT Id, Name FROM Account";
        let graph = QueryGraph::parse(q);
        assert_eq!(graph.base_object, "Account");
        assert!(graph.lookups.is_empty());
        assert!(graph.child_relationships.is_empty());
    }

    #[test]
    fn test_parse_lookups() {
        let q = "SELECT Id, Account.Name, Owner.Username FROM Contact";
        let graph = QueryGraph::parse(q);
        assert_eq!(graph.base_object, "Contact");
        assert!(graph.lookups.contains("Account"));
        assert!(graph.lookups.contains("Owner"));
        assert_eq!(graph.lookups.len(), 2);
    }

    #[test]
    fn test_parse_subqueries() {
        let q = "SELECT Id, (SELECT LastName FROM Contacts), (SELECT Id FROM Opportunities) FROM Account";
        let graph = QueryGraph::parse(q);
        assert_eq!(graph.base_object, "Account");
        assert!(graph.child_relationships.contains("Contacts"));
        assert!(graph.child_relationships.contains("Opportunities"));
        assert_eq!(graph.child_relationships.len(), 2);
    }

    #[test]
    fn test_parse_complex_query() {
        let q = "SELECT Id, Name, Account.Industry, (SELECT Title FROM Contacts) FROM Opportunity WHERE Amount > 1000";
        let graph = QueryGraph::parse(q);
        assert_eq!(graph.base_object, "Opportunity");
        assert!(graph.lookups.contains("Account"));
        assert!(graph.child_relationships.contains("Contacts"));
    }

    #[test]
    fn test_to_mermaid() {
        let mut graph = QueryGraph {
            base_object: "Opportunity".to_string(),
            lookups: HashSet::new(),
            child_relationships: HashSet::new(),
        };
        graph.lookups.insert("Account".to_string());
        graph.child_relationships.insert("Contacts".to_string());

        let mermaid = graph.to_mermaid();
        assert!(mermaid.starts_with("erDiagram\n"));
        assert!(mermaid.contains("Opportunity ||--o| Account : \"Account\""));
        assert!(mermaid.contains("Opportunity ||--o{ Contacts : \"Contacts\""));
    }

    #[test]
    fn test_to_mermaid_empty() {
        let graph = QueryGraph {
            base_object: "Account".to_string(),
            lookups: HashSet::new(),
            child_relationships: HashSet::new(),
        };
        let mermaid = graph.to_mermaid();
        assert_eq!(mermaid, "erDiagram\n  Account\n");
    }
}
