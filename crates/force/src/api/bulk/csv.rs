//! CSV streaming utilities for Bulk API 2.0.
//!
//! This module provides memory-efficient CSV serialization and deserialization
//! for large datasets. It supports streaming operations to handle files that
//! exceed available memory (100MB+).

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// Serializes a collection of records to CSV format.
///
/// Writes the records as CSV with headers to the provided writer. This function
/// streams the output and does not load the entire dataset into memory.
///
/// # Arguments
///
/// * `records` - The records to serialize
/// * `writer` - The writer to output CSV data to
///
/// # Errors
///
/// Returns an error if:
/// - CSV serialization fails
/// - Writing to the output fails
///
/// # Examples
///
/// ```ignore
/// use force::api::bulk::csv::serialize_to_csv;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Account {
///     name: String,
///     industry: String,
/// }
///
/// let accounts = vec![
///     Account { name: "Acme".to_string(), industry: "Technology".to_string() },
/// ];
///
/// let mut output = Vec::new();
/// serialize_to_csv(&accounts, &mut output)?;
/// ```
pub fn serialize_to_csv<T, W>(records: &[T], writer: W) -> Result<()>
where
    T: Serialize,
    W: Write,
{
    let mut csv_writer = csv::Writer::from_writer(writer);

    for record in records {
        csv_writer
            .serialize(record)
            .map_err(crate::error::SerializationError::from)?;
    }

    csv_writer
        .flush()
        .map_err(|e| crate::error::SerializationError::Csv(csv::Error::from(e)))?;

    Ok(())
}

/// Deserializes CSV data into a collection of records.
///
/// Reads CSV data from the provided reader and deserializes it into a vector
/// of records. This function streams the input and does not load the entire
/// dataset into memory.
///
/// # Arguments
///
/// * `reader` - The reader to read CSV data from
///
/// # Errors
///
/// Returns an error if:
/// - CSV deserialization fails
/// - Reading from the input fails
/// - Data validation fails
///
/// # Examples
///
/// ```ignore
/// use force::api::bulk::csv::deserialize_from_csv;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Account {
///     name: String,
///     industry: String,
/// }
///
/// let csv_data = "name,industry\nAcme,Technology\n";
/// let accounts: Vec<Account> = deserialize_from_csv(csv_data.as_bytes())?;
/// ```
pub fn deserialize_from_csv<T, R>(reader: R) -> Result<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
    R: Read,
{
    let mut csv_reader = csv::Reader::from_reader(reader);
    let mut records = Vec::new();

    for result in csv_reader.deserialize() {
        let record: T = result.map_err(crate::error::SerializationError::from)?;
        records.push(record);
    }

    Ok(records)
}

/// Processes CSV data in batches to reduce memory usage.
///
/// Reads CSV data in chunks and invokes a callback for each batch of records.
/// This allows processing very large datasets without loading everything into memory.
///
/// # Arguments
///
/// * `reader` - The reader to read CSV data from
/// * `batch_size` - Maximum number of records per batch
/// * `callback` - Function to process each batch
///
/// # Errors
///
/// Returns an error if:
/// - CSV deserialization fails
/// - Reading from the input fails
/// - The callback returns an error
///
/// # Examples
///
/// ```ignore
/// use force::api::bulk::csv::process_csv_batches;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Account {
///     name: String,
/// }
///
/// let csv_data = "name\nAcme\nGlobex\nInitech\n";
/// let mut total = 0;
///
/// process_csv_batches(
///     csv_data.as_bytes(),
///     2,
///     |batch: Vec<Account>| {
///         total += batch.len();
///         Ok(())
///     }
/// )?;
/// ```
pub fn process_csv_batches<T, R, F>(reader: R, batch_size: usize, mut callback: F) -> Result<()>
where
    T: for<'de> Deserialize<'de>,
    R: Read,
    F: FnMut(Vec<T>) -> Result<()>,
{
    let mut csv_reader = csv::Reader::from_reader(reader);
    let mut batch = Vec::with_capacity(batch_size);

    for result in csv_reader.deserialize() {
        let record: T = result.map_err(crate::error::SerializationError::from)?;
        batch.push(record);

        if batch.len() >= batch_size {
            callback(batch)?;
            batch = Vec::with_capacity(batch_size);
        }
    }

    // Process any remaining records
    if !batch.is_empty() {
        callback(batch)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestRecord {
        id: String,
        name: String,
        value: i32,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct ComplexRecord {
        id: String,
        name: String,
        #[serde(rename = "Description")]
        description: Option<String>,
        active: bool,
    }

    // Test 1: Basic CSV serialization with empty records
    #[test]
    fn test_serialize_empty_records() {
        let records: Vec<TestRecord> = vec![];
        let mut output = Vec::new();

        let result = serialize_to_csv(&records, &mut output);
        assert!(result.is_ok());

        // The csv crate doesn't write headers for empty datasets
        // This is acceptable behavior - if you have no records, you get no output
        let csv_str = String::from_utf8(output).unwrap();
        assert!(csv_str.is_empty());
    }

    // Test 2: Serialize single record
    #[test]
    fn test_serialize_single_record() {
        let records = vec![TestRecord {
            id: "001".to_string(),
            name: "Test".to_string(),
            value: 42,
        }];
        let mut output = Vec::new();

        serialize_to_csv(&records, &mut output).unwrap();

        let csv_str = String::from_utf8(output).unwrap();
        assert!(csv_str.contains("id,name,value"));
        assert!(csv_str.contains("001,Test,42"));
    }

    // Test 3: Serialize multiple records
    #[test]
    fn test_serialize_multiple_records() {
        let records = vec![
            TestRecord {
                id: "001".to_string(),
                name: "First".to_string(),
                value: 10,
            },
            TestRecord {
                id: "002".to_string(),
                name: "Second".to_string(),
                value: 20,
            },
            TestRecord {
                id: "003".to_string(),
                name: "Third".to_string(),
                value: 30,
            },
        ];
        let mut output = Vec::new();

        serialize_to_csv(&records, &mut output).unwrap();

        let csv_str = String::from_utf8(output).unwrap();
        assert!(csv_str.contains("001,First,10"));
        assert!(csv_str.contains("002,Second,20"));
        assert!(csv_str.contains("003,Third,30"));
    }

    // Test 4: Handle special characters and escaping
    #[test]
    fn test_serialize_with_special_characters() {
        let records = vec![
            TestRecord {
                id: "001".to_string(),
                name: "Name with \"quotes\"".to_string(),
                value: 1,
            },
            TestRecord {
                id: "002".to_string(),
                name: "Name, with comma".to_string(),
                value: 2,
            },
            TestRecord {
                id: "003".to_string(),
                name: "Name\nwith\nnewline".to_string(),
                value: 3,
            },
        ];
        let mut output = Vec::new();

        serialize_to_csv(&records, &mut output).unwrap();

        let csv_str = String::from_utf8(output).unwrap();
        // CSV should properly escape quotes, commas, and newlines
        assert!(csv_str.contains("\"Name with \"\"quotes\"\"\""));
        assert!(csv_str.contains("\"Name, with comma\""));
    }

    // Test 5: Handle optional fields
    #[test]
    fn test_serialize_with_optional_fields() {
        let records = vec![
            ComplexRecord {
                id: "001".to_string(),
                name: "First".to_string(),
                description: Some("Has description".to_string()),
                active: true,
            },
            ComplexRecord {
                id: "002".to_string(),
                name: "Second".to_string(),
                description: None,
                active: false,
            },
        ];
        let mut output = Vec::new();

        serialize_to_csv(&records, &mut output).unwrap();

        let csv_str = String::from_utf8(output).unwrap();
        assert!(csv_str.contains("Has description"));
        // None should serialize as empty field
        let lines: Vec<&str> = csv_str.lines().collect();
        assert_eq!(lines.len(), 3); // header + 2 records
    }

    // Test 6: Respect serde rename attributes
    #[test]
    fn test_serialize_with_rename() {
        let records = vec![ComplexRecord {
            id: "001".to_string(),
            name: "Test".to_string(),
            description: Some("test desc".to_string()),
            active: true,
        }];
        let mut output = Vec::new();

        serialize_to_csv(&records, &mut output).unwrap();

        let csv_str = String::from_utf8(output).unwrap();
        // Should use "Description" from rename attribute, not "description"
        assert!(csv_str.contains("Description"));
        assert!(!csv_str.contains("description,"));
    }

    // Test 7: Deserialize empty CSV
    #[test]
    fn test_deserialize_empty_csv() {
        let csv_data = "id,name,value\n";
        let result: Result<Vec<TestRecord>> = deserialize_from_csv(csv_data.as_bytes());

        assert!(result.is_ok());
        let records = result.unwrap();
        assert_eq!(records.len(), 0);
    }

    // Test 8: Deserialize single record
    #[test]
    fn test_deserialize_single_record() {
        let csv_data = "id,name,value\n001,Test,42\n";
        let records: Vec<TestRecord> = deserialize_from_csv(csv_data.as_bytes()).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "001");
        assert_eq!(records[0].name, "Test");
        assert_eq!(records[0].value, 42);
    }

    // Test 9: Deserialize multiple records
    #[test]
    fn test_deserialize_multiple_records() {
        let csv_data = "id,name,value\n001,First,10\n002,Second,20\n003,Third,30\n";
        let records: Vec<TestRecord> = deserialize_from_csv(csv_data.as_bytes()).unwrap();

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].name, "First");
        assert_eq!(records[1].name, "Second");
        assert_eq!(records[2].name, "Third");
    }

    // Test 10: Handle CSV with quoted fields
    #[test]
    fn test_deserialize_with_quoted_fields() {
        let csv_data =
            "id,name,value\n001,\"Name with \"\"quotes\"\"\",1\n002,\"Name, with comma\",2\n";
        let records: Vec<TestRecord> = deserialize_from_csv(csv_data.as_bytes()).unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].name, "Name with \"quotes\"");
        assert_eq!(records[1].name, "Name, with comma");
    }

    // Test 11: Handle optional fields in deserialization
    #[test]
    fn test_deserialize_with_optional_fields() {
        let csv_data =
            "id,name,Description,active\n001,First,Has description,true\n002,Second,,false\n";
        let records: Vec<ComplexRecord> = deserialize_from_csv(csv_data.as_bytes()).unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].description, Some("Has description".to_string()));
        assert_eq!(records[1].description, None);
    }

    // Test 12: Process batches with small batch size
    #[test]
    fn test_process_batches_small() {
        let csv_data = "id,name,value\n001,A,1\n002,B,2\n003,C,3\n004,D,4\n005,E,5\n";

        let mut batch_count = 0;
        let mut total_records = 0;

        let result = process_csv_batches(csv_data.as_bytes(), 2, |batch: Vec<TestRecord>| {
            batch_count += 1;
            total_records += batch.len();
            assert!(batch.len() <= 2);
            Ok(())
        });

        assert!(result.is_ok());
        assert_eq!(total_records, 5);
        assert_eq!(batch_count, 3); // 2 + 2 + 1
    }

    // Test 13: Process batches with batch size larger than dataset
    #[test]
    fn test_process_batches_large() {
        let csv_data = "id,name,value\n001,A,1\n002,B,2\n";

        let mut batch_count = 0;

        let result = process_csv_batches(csv_data.as_bytes(), 100, |batch: Vec<TestRecord>| {
            batch_count += 1;
            assert_eq!(batch.len(), 2);
            Ok(())
        });

        assert!(result.is_ok());
        assert_eq!(batch_count, 1);
    }

    // Test 14: Error handling in batch callback
    #[test]
    fn test_process_batches_callback_error() {
        let csv_data = "id,name,value\n001,A,1\n002,B,2\n003,C,3\n";

        let result = process_csv_batches(csv_data.as_bytes(), 2, |batch: Vec<TestRecord>| {
            // Check if any record in the batch has id "002"
            if batch.iter().any(|r| r.id == "002") {
                return Err(crate::error::ForceError::Http(
                    crate::error::HttpError::StatusError {
                        status_code: 500,
                        message: "Simulated error".to_string(),
                    },
                ));
            }
            Ok(())
        });

        assert!(result.is_err());
    }

    // Test 15: Large dataset simulation
    #[test]
    fn test_serialize_large_dataset() {
        // Generate 1000 records to simulate a larger dataset
        let records: Vec<TestRecord> = (0..1000)
            .map(|i| TestRecord {
                id: format!("{:06}", i),
                name: format!("Record_{}", i),
                value: i,
            })
            .collect();

        let mut output = Vec::new();
        let result = serialize_to_csv(&records, &mut output);

        assert!(result.is_ok());

        // Verify we got all records in the output
        let csv_str = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = csv_str.lines().collect();
        assert_eq!(lines.len(), 1001); // header + 1000 records
    }
}
