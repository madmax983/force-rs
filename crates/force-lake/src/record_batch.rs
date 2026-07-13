//! Assembly of Arrow [`RecordBatch`] values from Salesforce JSON records.
//!
//! Given an Arrow [`SchemaRef`] (derived from a Salesforce describe via
//! [`crate::schema_map`]) and a slice of Salesforce records as
//! [`serde_json::Value`], this module builds a columnar [`RecordBatch`] ready for
//! Parquet encoding.
//!
//! # Value conversion
//!
//! * Missing keys and JSON `null` become Arrow nulls.
//! * `Utf8` accepts JSON strings directly; other scalar JSON values are
//!   stringified.
//! * `Int32` / `Float64` / `Boolean` read the corresponding JSON scalar.
//! * `Decimal128(p, s)` parses the numeric string without floating-point loss.
//! * `Date32` parses Salesforce dates (`YYYY-MM-DD`) into day counts since the
//!   Unix epoch.
//! * `Timestamp(Microsecond, +00:00)` parses Salesforce datetimes such as
//!   `2024-01-15T10:30:00.000+0000` into UTC microseconds.
//! * `Binary` stores the raw UTF-8 bytes of the (base64) string value.

use std::sync::Arc;

use arrow_array::builder::{
    BinaryBuilder, BooleanBuilder, Date32Builder, Decimal128Builder, Float64Builder, Int32Builder,
    StringBuilder, Time64MicrosecondBuilder, TimestampMicrosecondBuilder,
};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::{DataType, SchemaRef, TimeUnit};
use chrono::{NaiveDate, NaiveTime, Timelike};
use serde_json::Value;

use crate::error::{LakeError, Result};

const MICROS_PER_SECOND: i64 = 1_000_000;

/// Builds a [`RecordBatch`] from Salesforce JSON records against an Arrow schema.
///
/// # Errors
///
/// Returns [`LakeError::RecordMapping`] if a value cannot be converted to the
/// column's declared Arrow type, or [`LakeError::Arrow`] if the assembled arrays
/// fail Arrow's batch validation.
pub fn build_record_batch(schema: &SchemaRef, records: &[Value]) -> Result<RecordBatch> {
    let mut columns: Vec<ArrayRef> = Vec::with_capacity(schema.fields().len());

    for field in schema.fields() {
        let name = field.name();
        let cells = records.iter().map(|record| record.get(name));
        let array = build_column(name, field.data_type(), cells)?;
        columns.push(array);
    }

    RecordBatch::try_new(Arc::clone(schema), columns).map_err(LakeError::from)
}

/// Builds a single Arrow column array from the JSON cells for one field.
fn build_column<'a, I>(name: &str, data_type: &DataType, cells: I) -> Result<ArrayRef>
where
    I: Iterator<Item = Option<&'a Value>>,
{
    match data_type {
        DataType::Utf8 => {
            let mut builder = StringBuilder::new();
            for cell in cells {
                match non_null(cell) {
                    None => builder.append_null(),
                    Some(Value::String(s)) => builder.append_value(s),
                    Some(other) => builder.append_value(other.to_string()),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Int32 => {
            let mut builder = Int32Builder::new();
            for cell in cells {
                match non_null(cell) {
                    None => builder.append_null(),
                    Some(value) => builder.append_value(parse_i32(name, value)?),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Float64 => {
            let mut builder = Float64Builder::new();
            for cell in cells {
                match non_null(cell) {
                    None => builder.append_null(),
                    Some(value) => builder.append_value(parse_f64(name, value)?),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Boolean => {
            let mut builder = BooleanBuilder::new();
            for cell in cells {
                match non_null(cell) {
                    None => builder.append_null(),
                    Some(Value::Bool(b)) => builder.append_value(*b),
                    Some(other) => {
                        return Err(LakeError::record_mapping(
                            name,
                            format!("expected boolean, got {other}"),
                        ));
                    }
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Decimal128(precision, scale) => {
            let mut builder = Decimal128Builder::new()
                .with_precision_and_scale(*precision, *scale)
                .map_err(LakeError::from)?;
            for cell in cells {
                match non_null(cell) {
                    None => builder.append_null(),
                    Some(value) => {
                        builder.append_value(parse_decimal(name, value, i32::from(*scale))?);
                    }
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Date32 => {
            let mut builder = Date32Builder::new();
            for cell in cells {
                match non_null(cell) {
                    None => builder.append_null(),
                    Some(value) => builder.append_value(parse_date32(name, value)?),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Time64(TimeUnit::Microsecond) => {
            let mut builder = Time64MicrosecondBuilder::new();
            for cell in cells {
                match non_null(cell) {
                    None => builder.append_null(),
                    Some(value) => builder.append_value(parse_time_micros(name, value)?),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Timestamp(TimeUnit::Microsecond, tz) => {
            let mut builder = TimestampMicrosecondBuilder::new();
            for cell in cells {
                match non_null(cell) {
                    None => builder.append_null(),
                    Some(value) => builder.append_value(parse_timestamp_micros(name, value)?),
                }
            }
            let array = match tz {
                Some(zone) => builder.finish().with_timezone(zone.clone()),
                None => builder.finish(),
            };
            Ok(Arc::new(array))
        }
        DataType::Binary => {
            let mut builder = BinaryBuilder::new();
            for cell in cells {
                match non_null(cell) {
                    None => builder.append_null(),
                    Some(Value::String(s)) => builder.append_value(s.as_bytes()),
                    Some(other) => builder.append_value(other.to_string().as_bytes()),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        other => Err(LakeError::record_mapping(
            name,
            format!("unsupported Arrow type `{other}` for Salesforce snapshot"),
        )),
    }
}

/// Returns `Some(value)` for a present, non-null JSON cell, else `None`.
fn non_null(cell: Option<&Value>) -> Option<&Value> {
    match cell {
        Some(Value::Null) | None => None,
        Some(value) => Some(value),
    }
}

fn parse_i32(name: &str, value: &Value) -> Result<i32> {
    value
        .as_i64()
        .and_then(|v| i32::try_from(v).ok())
        .ok_or_else(|| LakeError::record_mapping(name, format!("expected 32-bit int, got {value}")))
}

fn parse_f64(name: &str, value: &Value) -> Result<f64> {
    value
        .as_f64()
        .ok_or_else(|| LakeError::record_mapping(name, format!("expected float, got {value}")))
}

/// Parses a decimal JSON scalar into an unscaled `i128` for `Decimal128`.
///
/// Avoids floating-point conversion by shifting the decimal point directly.
fn parse_decimal(name: &str, value: &Value, scale: i32) -> Result<i128> {
    let text = match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        other => {
            return Err(LakeError::record_mapping(
                name,
                format!("expected decimal, got {other}"),
            ));
        }
    };
    parse_decimal_str(&text, scale)
        .ok_or_else(|| LakeError::record_mapping(name, format!("invalid decimal `{text}`")))
}

fn parse_decimal_str(text: &str, scale: i32) -> Option<i128> {
    let scale = usize::try_from(scale).ok()?;
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (int_part, frac_part) = match digits.split_once('.') {
        Some((i, f)) => (i, f),
        None => (digits, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }

    let mut scaled_frac = String::with_capacity(scale);
    for i in 0..scale {
        scaled_frac.push(frac_part.as_bytes().get(i).map_or('0', |b| *b as char));
    }
    let combined = format!("{int_part}{scaled_frac}");
    let mut magnitude: i128 = combined.parse().ok()?;
    if negative {
        magnitude = -magnitude;
    }
    Some(magnitude)
}

/// Parses a Salesforce date (`YYYY-MM-DD`) into days since the Unix epoch.
fn parse_date32(name: &str, value: &Value) -> Result<i32> {
    let text = value.as_str().ok_or_else(|| {
        LakeError::record_mapping(name, format!("expected date string, got {value}"))
    })?;
    let date = NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map_err(|e| LakeError::record_mapping(name, format!("invalid date `{text}`: {e}")))?;
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)
        .ok_or_else(|| LakeError::record_mapping(name, "epoch date construction failed"))?;
    let days = (date - epoch).num_days();
    i32::try_from(days)
        .map_err(|_| LakeError::record_mapping(name, format!("date `{text}` out of range")))
}

/// Parses a Salesforce time (`HH:MM:SS(.fff)?Z?`) into microseconds past midnight.
fn parse_time_micros(name: &str, value: &Value) -> Result<i64> {
    let text = value.as_str().ok_or_else(|| {
        LakeError::record_mapping(name, format!("expected time string, got {value}"))
    })?;
    let trimmed = text.trim_end_matches('Z');
    let time = NaiveTime::parse_from_str(trimmed, "%H:%M:%S%.f")
        .or_else(|_| NaiveTime::parse_from_str(trimmed, "%H:%M:%S"))
        .map_err(|e| LakeError::record_mapping(name, format!("invalid time `{text}`: {e}")))?;
    let secs = i64::from(time.num_seconds_from_midnight());
    let micros = i64::from(time.nanosecond() / 1_000);
    Ok(secs * MICROS_PER_SECOND + micros)
}

/// Parses a Salesforce datetime into UTC microseconds since the Unix epoch.
fn parse_timestamp_micros(name: &str, value: &Value) -> Result<i64> {
    let text = value.as_str().ok_or_else(|| {
        LakeError::record_mapping(name, format!("expected datetime string, got {value}"))
    })?;
    let parsed = chrono::DateTime::parse_from_rfc3339(text)
        .or_else(|_| chrono::DateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f%z"))
        .map_err(|e| LakeError::record_mapping(name, format!("invalid datetime `{text}`: {e}")))?;
    Ok(parsed.timestamp_micros())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{
        Array, BooleanArray, Date32Array, Decimal128Array, Int32Array, StringArray,
        TimestampMicrosecondArray,
    };
    use arrow_schema::{Field, Schema};
    use serde_json::json;

    fn schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("Id", DataType::Utf8, false),
            Field::new("Amount", DataType::Decimal128(18, 2), true),
            Field::new("Employees", DataType::Int32, true),
            Field::new("IsWon", DataType::Boolean, true),
            Field::new("CloseDate", DataType::Date32, true),
            Field::new(
                "CreatedDate",
                DataType::Timestamp(TimeUnit::Microsecond, Some("+00:00".into())),
                true,
            ),
        ]))
    }

    #[test]
    fn builds_batch_with_values_and_nulls() {
        let records = vec![
            json!({
                "Id": "001aaa",
                "Amount": "1234.56",
                "Employees": 42,
                "IsWon": true,
                "CloseDate": "2024-01-15",
                "CreatedDate": "2024-01-15T10:30:00.000+0000"
            }),
            json!({
                "Id": "001bbb",
                "Amount": null,
                "IsWon": false,
                "CreatedDate": "2024-06-30T23:59:59.500Z"
            }),
        ];

        let batch = build_record_batch(&schema(), &records).expect("batch builds");
        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.num_columns(), 6);

        let ids = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("string column");
        assert_eq!(ids.value(0), "001aaa");
        assert_eq!(ids.value(1), "001bbb");

        let amounts = batch
            .column(1)
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .expect("decimal column");
        // 1234.56 scaled by 10^2 = 123456
        assert_eq!(amounts.value(0), 123_456_i128);
        assert!(amounts.is_null(1));

        let employees = batch
            .column(2)
            .as_any()
            .downcast_ref::<Int32Array>()
            .expect("int column");
        assert_eq!(employees.value(0), 42);
        assert!(employees.is_null(1));

        let won = batch
            .column(3)
            .as_any()
            .downcast_ref::<BooleanArray>()
            .expect("bool column");
        assert!(won.value(0));
        assert!(!won.value(1));

        let close = batch
            .column(4)
            .as_any()
            .downcast_ref::<Date32Array>()
            .expect("date column");
        // 2024-01-15 is 19737 days after the Unix epoch.
        assert_eq!(close.value(0), 19_737);
        assert!(close.is_null(1));

        let created = batch
            .column(5)
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .expect("timestamp column");
        // 2024-01-15T10:30:00Z
        assert_eq!(created.value(0), 1_705_314_600_000_000);
        // .500 fractional second preserved.
        assert_eq!(created.value(1) % MICROS_PER_SECOND, 500_000);
    }

    #[test]
    fn negative_decimal_parses() {
        assert_eq!(parse_decimal_str("-12.5", 2), Some(-1250));
        assert_eq!(parse_decimal_str("0.01", 2), Some(1));
        assert_eq!(parse_decimal_str("100", 2), Some(10_000));
        assert_eq!(parse_decimal_str("abc", 2), None);
    }
}
