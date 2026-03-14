//! Markdown Exporter for generic JSON records.
//!
//! This module provides a simple utility to convert lists of JSON objects
//! (like those returned from a dynamic SOQL query) into formatted Markdown tables.
//!
//! # Example
//!
//! ```no_run
//! # use force::client::ForceClientBuilder;
//! # use force::experimental::MarkdownExporter;
//! # use force::auth::ClientCredentials;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! let result = client.rest().query::<serde_json::Value>("SELECT Id, Name FROM Account LIMIT 2").await?;
//!
//! let exporter = MarkdownExporter::new();
//! let md = exporter.export(&result.records);
//!
//! println!("{}", md);
//! # Ok(())
//! # }
//! ```

use serde_json::Value;

/// Exporter that converts generic JSON records into Markdown tables.
#[derive(Debug, Default)]
pub struct MarkdownExporter;

impl MarkdownExporter {
    /// Creates a new `MarkdownExporter`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Converts a list of JSON records into a Markdown table.
    ///
    /// It automatically extracts keys from the first record to use as headers,
    /// skipping the `attributes` key (which contains Salesforce-specific metadata).
    ///
    /// # Arguments
    ///
    /// * `records` - A slice of `serde_json::Value` records.
    ///
    /// # Returns
    ///
    /// A formatted string representing a Markdown table, or an empty string if
    /// the records list is empty.
    #[must_use]
    pub fn export(&self, records: &[Value]) -> String {
        if records.is_empty() {
            return String::new();
        }

        // Extract keys from the first record, skipping 'attributes'
        let mut keys: Vec<String> = Vec::new();
        if let Some(Value::Object(map)) = records.first() {
            for key in map.keys() {
                if key != "attributes" {
                    keys.push(key.clone());
                }
            }
        }

        if keys.is_empty() {
            return String::new();
        }

        let mut md = String::new();

        // 1. Headers
        md.push('|');
        for key in &keys {
            md.push_str(&format!(" {} |", key));
        }
        md.push('\n');

        // 2. Separator
        md.push('|');
        for _ in &keys {
            md.push_str("---|");
        }
        md.push('\n');

        // 3. Rows
        for record in records {
            if let Value::Object(map) = record {
                md.push('|');
                for key in &keys {
                    let cell_value = match map.get(key) {
                        Some(Value::String(s)) => s.clone(),
                        Some(Value::Null) | None => String::new(),
                        Some(other) => other.to_string(),
                    };
                    md.push_str(&format!(" {} |", cell_value));
                }
                md.push('\n');
            }
        }

        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_empty_records() {
        let exporter = MarkdownExporter::new();
        let md = exporter.export(&[]);
        assert_eq!(md, "");
    }

    #[test]
    fn test_export_basic_records() {
        let records = vec![
            json!({
                "attributes": { "type": "Account", "url": "/services/data/v60.0/sobjects/Account/001" },
                "Id": "001A00000000000",
                "Name": "Acme Corp",
                "AnnualRevenue": 1_000_000
            }),
            json!({
                "attributes": { "type": "Account", "url": "/services/data/v60.0/sobjects/Account/002" },
                "Id": "001A00000000001",
                "Name": "Global Systems",
                "AnnualRevenue": null
            }),
        ];

        let exporter = MarkdownExporter::new();
        let md = exporter.export(&records);

        // Keys order is not strictly guaranteed in serde_json::Map depending on how it's constructed,
        // but json! macro usually preserves it in standard configs, or we can just check substring presence.

        assert!(md.contains("| Id |"));
        assert!(md.contains("| Name |"));
        assert!(md.contains("| AnnualRevenue |"));
        assert!(!md.contains("attributes"));

        assert!(md.contains("| 001A00000000000 | Acme Corp | 1000000 |") ||
                md.contains("| Acme Corp | 1000000 | 001A00000000000 |") ||
                md.contains("| 1000000 | 001A00000000000 | Acme Corp |") ||
                md.contains("| 001A00000000000 | 1000000 | Acme Corp |") ||
                md.contains("| Acme Corp | 001A00000000000 | 1000000 |") ||
                md.contains("| 1000000 | Acme Corp | 001A00000000000 |"));

        // Check for the null handling (empty string)
        assert!(md.contains("| 001A00000000001 | Global Systems |  |") ||
                md.contains("| Global Systems |  | 001A00000000001 |") ||
                md.contains("|  | 001A00000000001 | Global Systems |") ||
                md.contains("| 001A00000000001 |  | Global Systems |") ||
                md.contains("| Global Systems | 001A00000000001 |  |") ||
                md.contains("|  | Global Systems | 001A00000000001 |"));
    }
}
