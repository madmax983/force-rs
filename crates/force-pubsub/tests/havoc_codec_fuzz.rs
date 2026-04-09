//! Fuzz testing for Avro decoding.

#![allow(clippy::unwrap_used)]

use apache_avro::Schema;
use force_pubsub::codec::decode_avro;
use proptest::prelude::*;

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

proptest! {
    #[test]
    fn havoc_decode_avro_no_panics(
        bytes in proptest::collection::vec(any::<u8>(), 0..1024),
    ) {
        let schema = Schema::parse_str(SIMPLE_SCHEMA).unwrap();
        let _ = decode_avro(&schema, &bytes);
    }
}
