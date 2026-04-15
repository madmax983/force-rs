fn main() {
    let schema = apache_avro::Schema::parse_str(r#"{
        "type": "record",
        "name": "TestEvent",
        "fields": [
            {"name": "id", "type": "string"},
            {"name": "amount", "type": "double"}
        ]
    }"#).unwrap();
    let mut map = serde_json::Map::new();
    map.insert("id".to_string(), serde_json::Value::String("event-001".to_string()));
    map.insert("amount".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(99.5).unwrap()));
    let payload = serde_json::Value::Object(map);
    let avro_value = apache_avro::to_value(payload).unwrap();
    println!("{:?}", avro_value);
}
