//! Avro codec utilities for encoding and decoding Salesforce Pub/Sub events.
//!
//! Salesforce Pub/Sub events are encoded as Avro binary (not JSON).
//! Each event carries a `schema_id` in its header that identifies the Avro
//! schema required for decoding.

use apache_avro::{Schema, from_value, to_avro_datum};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{PubSubError, Result};

fn avro_value_from_bytes(schema: &Schema, bytes: &[u8]) -> Result<apache_avro::types::Value> {
    apache_avro::from_avro_datum(schema, &mut std::io::Cursor::new(bytes), None)
        .map_err(|e| PubSubError::Avro(e.to_string()))
}

/// Decode Avro-binary bytes into a [`serde_json::Value`] using the given schema.
///
/// The schema must match the `schema_id` on the event header.
///
/// # Errors
///
/// Returns [`PubSubError::Avro`] if the bytes cannot be decoded with the
/// provided schema.
pub fn decode_avro(schema: &Schema, bytes: &[u8]) -> Result<Value> {
    let value = avro_value_from_bytes(schema, bytes)?;
    from_value::<Value>(&value).map_err(|e| PubSubError::Avro(e.to_string()))
}

/// Decode Avro-binary bytes directly into a typed struct `T` using the given schema.
///
/// This is a **single-step** Avro → `T` decode. It is a public utility for callers
/// who already hold a schema and bytes and want to avoid the two-step
/// (`decode_avro` → `serde_json::from_value`) path used internally by the typed
/// subscribe stream. Both produce identical results; this variant is marginally
/// cheaper because it skips the intermediate [`serde_json::Value`] allocation.
///
/// The typed subscribe stream (`subscribe_typed` / `subscribe_typed_dynamic`) does
/// not use this function because it is built on top of the dynamic stream
/// (which decodes to `Value` first) so that both variants share one subscribe
/// loop implementation. Callers who own raw event bytes and want zero-overhead
/// typed decoding should prefer this function.
///
/// # Errors
///
/// Returns [`PubSubError::Avro`] if the bytes cannot be decoded or deserialized
/// into `T`.
pub fn decode_avro_typed<T: for<'de> Deserialize<'de>>(schema: &Schema, bytes: &[u8]) -> Result<T> {
    let value = avro_value_from_bytes(schema, bytes)?;
    from_value::<T>(&value).map_err(|e| PubSubError::Avro(e.to_string()))
}

/// Encode a serializable value to Avro binary using the given schema.
///
/// Used when publishing events to the Salesforce Pub/Sub API.
///
/// # Errors
///
/// Returns [`PubSubError::Avro`] if the value cannot be serialized or resolved
/// against the provided schema.
pub fn encode_avro<T: Serialize>(schema: &Schema, value: &T) -> Result<Vec<u8>> {
    let avro_value = apache_avro::to_value(value).map_err(|e| PubSubError::Avro(e.to_string()))?;
    let resolved = avro_value
        .resolve(schema)
        .map_err(|e| PubSubError::Avro(e.to_string()))?;
    to_avro_datum(schema, resolved).map_err(|e| PubSubError::Avro(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use apache_avro::Schema;

    const SIMPLE_SCHEMA: &str = r#"
    {
        "type": "record",
        "name": "TestEvent",
        "fields": [
            {"name": "id", "type": "string"},
            {"name": "amount", "type": "double"}
        ]
    }
    "#;

    #[test]
    fn test_encode_decode_roundtrip_dynamic() {
        let schema = Schema::parse_str(SIMPLE_SCHEMA).expect("valid schema");
        let payload = serde_json::json!({
            "id": "event-001",
            "amount": 99.5
        });

        let encoded = encode_avro(&schema, &payload).expect("encode succeeds");
        assert!(!encoded.is_empty());

        let decoded: Value = decode_avro(&schema, &encoded).expect("decode succeeds");
        assert_eq!(decoded["id"], "event-001");
        assert!((decoded["amount"].as_f64().unwrap() - 99.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_encode_decode_roundtrip_typed() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct TestEvent {
            id: String,
            amount: f64,
        }

        let schema = Schema::parse_str(SIMPLE_SCHEMA).expect("valid schema");
        let event = TestEvent {
            id: "event-002".to_string(),
            amount: 42.0,
        };

        let encoded = encode_avro(&schema, &event).expect("encode succeeds");
        let decoded: TestEvent = decode_avro_typed(&schema, &encoded).expect("decode succeeds");

        assert_eq!(decoded.id, "event-002");
        assert!((decoded.amount - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_decode_invalid_bytes_returns_error() {
        let schema = Schema::parse_str(SIMPLE_SCHEMA).expect("valid schema");
        let garbage = vec![0xFF, 0xFE, 0xFD];
        let result = decode_avro(&schema, &garbage);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PubSubError::Avro(_)));
    }
}
