# Bolt's Journal ⚡

**[Optimizing Optional Telemetry]**
**Learning:** Constructing structs with `String` fields for telemetry events (e.g. `RequestCompletion`) allocates memory even if the event is never consumed (no hooks registered).
**Action:** Use `Option<String>` in the context struct and only populate it if telemetry hooks are active. Pass a capture flag to the context constructor.
