//! In-memory Parquet encoding of Arrow record batches.
//!
//! Snapshots are staged as Parquet byte buffers before being handed to a
//! [`crate::catalog::LakeCatalog`] as Iceberg data files. This module wraps
//! [`parquet::arrow::ArrowWriter`] to encode a set of [`RecordBatch`] values that
//! all share one Arrow [`SchemaRef`] into a single `Vec<u8>`.

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use crate::error::{LakeError, Result};

/// Encodes record batches into a single in-memory Parquet file.
///
/// All batches must share `schema`. Returns the encoded Parquet bytes.
///
/// # Errors
///
/// Returns [`LakeError::Parquet`] if a batch does not match `schema` or if
/// encoding fails.
pub fn write_parquet(schema: &SchemaRef, batches: &[RecordBatch]) -> Result<Vec<u8>> {
    let mut buffer: Vec<u8> = Vec::new();
    let properties = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();

    let mut writer = ArrowWriter::try_new(&mut buffer, schema.clone(), Some(properties))
        .map_err(LakeError::from)?;
    for batch in batches {
        writer.write(batch).map_err(LakeError::from)?;
    }
    writer.close().map_err(LakeError::from)?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use arrow_array::{Int32Array, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    fn sample() -> (SchemaRef, RecordBatch) {
        let schema: SchemaRef = Arc::new(Schema::new(vec![
            Field::new("Id", DataType::Utf8, false),
            Field::new("Employees", DataType::Int32, true),
        ]));
        let ids = StringArray::from(vec!["001aaa", "001bbb", "001ccc"]);
        let employees = Int32Array::from(vec![Some(10), None, Some(30)]);
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(ids), Arc::new(employees)],
        )
        .expect("batch builds");
        (schema, batch)
    }

    #[test]
    fn round_trips_through_parquet() {
        let (schema, batch) = sample();
        let bytes = write_parquet(&schema, std::slice::from_ref(&batch)).expect("parquet writes");
        assert!(!bytes.is_empty());

        let reader = ParquetRecordBatchReaderBuilder::try_new(bytes::Bytes::from(bytes))
            .expect("reader builds")
            .build()
            .expect("reader");
        let mut total_rows = 0;
        let mut first_id = None;
        for maybe in reader {
            let read = maybe.expect("batch reads");
            if first_id.is_none() {
                let ids = read
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("string column");
                first_id = Some(ids.value(0).to_owned());
            }
            total_rows += read.num_rows();
        }
        assert_eq!(total_rows, 3);
        assert_eq!(first_id.as_deref(), Some("001aaa"));
    }
}
