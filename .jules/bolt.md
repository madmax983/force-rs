# Bolt's Journal ⚡

**[Optimizing Optional Telemetry]**
**Learning:** Constructing structs with `String` fields for telemetry events (e.g. `RequestCompletion`) allocates memory even if the event is never consumed (no hooks registered).
**Action:** Use `Option<String>` in the context struct and only populate it if telemetry hooks are active. Pass a capture flag to the context constructor.

**[Zero-Cost JSON Deserialization]**
**Learning:** `serde_json::from_value` requires taking ownership of `serde_json::Value`, forcing an expensive `.clone()` when the source is borrowed. However, `&Value` implements `serde::Deserializer`, so `T::deserialize(value)` can be used to deserialize directly from a reference without cloning the tree.
**Action:** Always prefer `T::deserialize(value)` over `serde_json::from_value(value.clone())` when reading fields from a borrowed JSON value to avoid heap allocations.
