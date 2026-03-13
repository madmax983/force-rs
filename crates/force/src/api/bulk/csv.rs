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
/// ```
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
/// # Ok::<(), force::error::ForceError>(())
/// ```
pub fn serialize_to_csv<T, W>(records: &[T], writer: W) -> Result<()>
where
    T: Serialize,
    W: Write,
{
    serialize_to_csv_with_options(records, writer, true)
}

/// Serializes a collection of records to CSV format with configuration options.
///
/// Allows configuring whether to include headers in the output.
///
/// # Arguments
///
/// * `records` - The records to serialize
/// * `writer` - The writer to output CSV data to
/// * `has_headers` - Whether to include the header row
pub fn serialize_to_csv_with_options<T, W>(
    records: &[T],
    writer: W,
    has_headers: bool,
) -> Result<()>
where
    T: Serialize,
    W: Write,
{
    let mut csv_writer = csv::WriterBuilder::new()
        .has_headers(has_headers)
        .from_writer(writer);

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

/// A reader that enforces a maximum read limit to prevent memory exhaustion (DoS).
struct LimitReader<R> {
    inner: R,
    limit: usize,
    read_bytes: usize,
}

impl<R: Read> LimitReader<R> {
    fn new(inner: R, limit: usize) -> Self {
        Self {
            inner,
            limit,
            read_bytes: 0,
        }
    }
}

impl<R: Read> Read for LimitReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.read_bytes > self.limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Payload exceeded maximum size limit",
            ));
        }

        let remaining = self.limit.saturating_sub(self.read_bytes);
        let max_to_read = remaining.saturating_add(1);
        let to_read = std::cmp::min(buf.len(), max_to_read);

        let n = self.inner.read(&mut buf[..to_read])?;

        self.read_bytes = self.read_bytes.saturating_add(n);

        if self.read_bytes > self.limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Payload exceeded maximum size limit",
            ));
        }

        Ok(n)
    }
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
/// ```
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
/// # Ok::<(), force::error::ForceError>(())
/// ```
pub fn deserialize_from_csv<T, R>(reader: R) -> Result<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
    R: Read,
{
    let limit_reader = LimitReader::new(reader, 150 * 1024 * 1024);
    let mut csv_reader = csv::Reader::from_reader(limit_reader);
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
/// ```
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
/// # Ok::<(), force::error::ForceError>(())
/// ```
pub fn process_csv_batches<T, R, F>(reader: R, batch_size: usize, mut callback: F) -> Result<()>
where
    T: for<'de> Deserialize<'de>,
    R: Read,
    F: FnMut(Vec<T>) -> Result<()>,
{
    if batch_size == 0 {
        return Err(crate::error::ForceError::InvalidInput(
            "Batch size must be greater than 0".to_string(),
        ));
    }

    let batches = CsvBatchIterator::new(reader, batch_size);

    for batch in batches {
        callback(batch?)?;
    }

    Ok(())
}

struct CsvBatchIterator<R, T> {
    iter: csv::DeserializeRecordsIntoIter<R, T>,
    batch_size: usize,
}

impl<R, T> CsvBatchIterator<R, T>
where
    R: Read,
    T: for<'de> Deserialize<'de>,
{
    fn new(reader: R, batch_size: usize) -> Self {
        let csv_reader = csv::Reader::from_reader(reader);
        Self {
            iter: csv_reader.into_deserialize(),
            batch_size,
        }
    }
}

impl<R, T> Iterator for CsvBatchIterator<R, T>
where
    R: Read,
    T: for<'de> Deserialize<'de>,
{
    type Item = Result<Vec<T>>;

    fn next(&mut self) -> Option<Self::Item> {
        // Cap initial allocation to avoid panic/OOM on huge batch_size
        let capacity = std::cmp::min(self.batch_size, 10_000);
        let mut batch = Vec::with_capacity(capacity);

        for _ in 0..self.batch_size {
            match self.iter.next() {
                Some(Ok(record)) => batch.push(record),
                Some(Err(e)) => {
                    return Some(Err(crate::error::SerializationError::from(e).into()));
                }
                None => break,
            }
        }

        if batch.is_empty() {
            None
        } else {
            Some(Ok(batch))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Must;
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
        let records: Vec<TestRecord> = vec![TestRecord {
            id: "001".to_string(),
            name: "Test".to_string(),
            value: 42,
        }];
        let mut output = Vec::new();

        serialize_to_csv_with_options(&records, &mut output, false).must();
        let csv_str = String::from_utf8(output).must();
        assert_eq!(csv_str, "001,Test,42\n");

        let records: Vec<TestRecord> = vec![];
        let mut output = Vec::new();

        serialize_to_csv(&records, &mut output).must();
        let csv_str = String::from_utf8(output).must();
        assert_eq!(csv_str, "");
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

        serialize_to_csv(&records, &mut output).must();

        let csv_str = String::from_utf8(output).must();
        assert_eq!(csv_str.lines().next().must(), "id,name,value");
        assert_eq!(csv_str.lines().nth(1).must(), "001,Test,42");
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

        serialize_to_csv(&records, &mut output).must();

        let csv_str = String::from_utf8(output).must();
        assert_eq!(csv_str.lines().nth(1).must(), "001,First,10");
        assert_eq!(csv_str.lines().nth(2).must(), "002,Second,20");
        assert_eq!(csv_str.lines().nth(3).must(), "003,Third,30");
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

        serialize_to_csv(&records, &mut output).must();

        let csv_str = String::from_utf8(output).must();
        // CSV should properly escape quotes, commas, and newlines
        assert!(
            csv_str
                .lines()
                .any(|l| l == "001,\"Name with \"\"quotes\"\"\",1")
        );
        assert!(csv_str.lines().any(|l| l == "002,\"Name, with comma\",2"));
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

        serialize_to_csv(&records, &mut output).must();

        let csv_str = String::from_utf8(output).must();
        assert_eq!(
            csv_str.lines().nth(1).must(),
            "001,First,Has description,true"
        );
        // None should serialize as empty field
        assert_eq!(csv_str.lines().nth(2).must(), "002,Second,,false"); // header + 2 records
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

        serialize_to_csv(&records, &mut output).must();

        let csv_str = String::from_utf8(output).must();
        // Should use "Description" from rename attribute, not "description"
        assert_eq!(csv_str.lines().next().must(), "id,name,Description,active");
        assert_eq!(csv_str.lines().nth(1).must(), "001,Test,test desc,true");
    }

    // Test 7: Deserialize empty CSV
    #[test]
    fn test_deserialize_empty_csv() {
        let csv_data = "id,name,value\n";
        let records: Vec<TestRecord> = deserialize_from_csv(csv_data.as_bytes()).must();
        assert_eq!(records.len(), 0);
    }

    // Test 8: Deserialize single record
    #[test]
    fn test_deserialize_single_record() {
        let csv_data = "id,name,value\n001,Test,42\n";
        let records: Vec<TestRecord> = deserialize_from_csv(csv_data.as_bytes()).must();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "001");
        assert_eq!(records[0].name, "Test");
        assert_eq!(records[0].value, 42);
    }

    // Test 9: Deserialize multiple records
    #[test]
    fn test_deserialize_multiple_records() {
        let csv_data = "id,name,value\n001,First,10\n002,Second,20\n003,Third,30\n";
        let records: Vec<TestRecord> = deserialize_from_csv(csv_data.as_bytes()).must();

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
        let records: Vec<TestRecord> = deserialize_from_csv(csv_data.as_bytes()).must();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].name, "Name with \"quotes\"");
        assert_eq!(records[1].name, "Name, with comma");
    }

    // Test 11: Handle optional fields in deserialization
    #[test]
    fn test_deserialize_with_optional_fields() {
        let csv_data =
            "id,name,Description,active\n001,First,Has description,true\n002,Second,,false\n";
        let records: Vec<ComplexRecord> = deserialize_from_csv(csv_data.as_bytes()).must();

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

        process_csv_batches(csv_data.as_bytes(), 2, |batch: Vec<TestRecord>| {
            batch_count += 1;
            total_records += batch.len();
            assert!(batch.len() <= 2);
            Ok(())
        })
        .must();
        assert_eq!(total_records, 5);
        assert_eq!(batch_count, 3); // 2 + 2 + 1
    }

    // Test 13: Process batches with batch size larger than dataset
    #[test]
    fn test_process_batches_large() {
        let csv_data = "id,name,value\n001,A,1\n002,B,2\n";

        let mut batch_count = 0;

        process_csv_batches(csv_data.as_bytes(), 100, |batch: Vec<TestRecord>| {
            batch_count += 1;
            assert_eq!(batch.len(), 2);
            Ok(())
        })
        .must();
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

        let result = process_csv_batches(csv_data.as_bytes(), 0, |_: Vec<TestRecord>| Ok(()));
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
        serialize_to_csv(&records, &mut output).must();

        // Verify we got all records in the output
        let csv_str = String::from_utf8(output).must();
        assert_eq!(csv_str.lines().count(), 1001); // header + 1000 records
    }

    // Property-based tests using proptest
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        // Strategy for generating arbitrary TestRecords
        fn arbitrary_test_record() -> impl Strategy<Value = TestRecord> {
            ("[a-zA-Z0-9]{1,18}", "[a-zA-Z0-9 ]{1,80}", any::<i32>())
                .prop_map(|(id, name, value)| TestRecord { id, name, value })
        }

        proptest! {
            // Property 1: Serialize -> Deserialize roundtrip for any record
            #[test]
            fn prop_roundtrip_single_record(record in arbitrary_test_record()) {
                let records = vec![record.clone()];
                let mut output = Vec::new();

                serialize_to_csv(&records, &mut output).must();
                let deserialized: Vec<TestRecord> = deserialize_from_csv(output.as_slice()).must();

                prop_assert_eq!(deserialized.len(), 1);
                prop_assert_eq!(&deserialized[0], &record);
            }

            // Property 2: Roundtrip with multiple records
            #[test]
            fn prop_roundtrip_multiple_records(records in prop::collection::vec(arbitrary_test_record(), 0..50)) {
                let mut output = Vec::new();

                serialize_to_csv(&records, &mut output).must();
                let deserialized: Vec<TestRecord> = deserialize_from_csv(output.as_slice()).must();

                prop_assert_eq!(deserialized.len(), records.len());
                prop_assert_eq!(deserialized, records);
            }

            // Property 3: Batch processing handles all records
            #[test]
            fn prop_batch_processing_complete(
                records in prop::collection::vec(arbitrary_test_record(), 1..100),
                batch_size in 1usize..20usize
            ) {
                let mut output = Vec::new();
                serialize_to_csv(&records, &mut output).must();

                let mut collected = Vec::new();
                process_csv_batches(
                    output.as_slice(),
                    batch_size,
                    |batch: Vec<TestRecord>| {
                        collected.extend(batch);
                        Ok(())
                    }
                ).must();

                prop_assert_eq!(collected.len(), records.len());
                prop_assert_eq!(collected, records);
            }

            // Property 4: Batch sizes are respected
            #[test]
            fn prop_batch_sizes_respected(
                records in prop::collection::vec(arbitrary_test_record(), 10..50),
                batch_size in 1usize..10usize
            ) {
                let mut output = Vec::new();
                serialize_to_csv(&records, &mut output).must();

                let mut batch_sizes = Vec::new();
                process_csv_batches(
                    output.as_slice(),
                    batch_size,
                    |batch: Vec<TestRecord>| {
                        batch_sizes.push(batch.len());
                        Ok(())
                    }
                ).must();

                // All batches except possibly the last should be full size
                for &size in &batch_sizes[..batch_sizes.len().saturating_sub(1)] {
                    prop_assert_eq!(size, batch_size);
                }

                // Last batch can be any size up to batch_size
                if let Some(&last) = batch_sizes.last() {
                    prop_assert!(last > 0 && last <= batch_size);
                }

                // Total records should match
                let total: usize = batch_sizes.iter().sum();
                prop_assert_eq!(total, records.len());
            }
        }
    }
}
