# UI API

Layout-aware record data, object metadata, list views, actions, lookups, and
favorites via `/services/data/vXX.0/ui-api/`. Unlike the REST API (raw SObject
data), the UI API returns presentation-ready structures: display values, layout
sections, and user-context-aware field visibility.

- **Feature flag:** `ui`
- **Accessor:** `client.ui()` → `UiHandler`

```toml
force = { version = "...", features = ["ui"] }
```

## Records

```rust
let ui = client.ui();

// Layout-aware aggregation (record data + object info + layout) for one or more IDs
let record_ui = ui.record_ui(&["001000000000001AAA"], None, None).await?;

// Single record, optional field projection
let account = ui.get_record("001000000000001AAA", Some(&["Name", "Industry"])).await?;

// CRUD (create_record / update_record take typed *Input structs)
ui.delete_record("001000000000001AAA").await?;
```

Other record methods: `get_records_batch`, `create_record`, `update_record`,
`create_defaults`, `clone_defaults`.

## Metadata, layouts, list views

```rust
let info    = ui.object_info("Account").await?;          // object metadata
let batch   = ui.object_infos_batch(&["Account", "Contact"]).await?;
let layout  = ui.layout("Account", None, None).await?;   // page layout
let records = ui.list_records("Account", "MyListView", None).await?;
```

List-view methods: `list_ui`, `list_views`, `list_records`, `list_info`.
Also: `record_actions`, `lookup` / `filtered_lookup`, and favorites
(`get_favorites`, `create_favorite`, `update_favorite`, `delete_favorite`).

## See also

- [ADR-020 — UI API design](../../adr/020-ui-api-design.md)
- Example: [`crates/force/examples/ui_api.rs`](../../../crates/force/examples/ui_api.rs)
- Rustdoc: `cargo doc --no-deps --features ui --open` → `force::api::ui`
