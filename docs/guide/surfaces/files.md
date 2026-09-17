# Files API

Upload and download Salesforce Files (`ContentVersion`) and link them to records
(`ContentDocumentLink`), without hand-rolling multipart requests.

- **Feature flag:** `files` (depends on `rest`)
- **Accessor:** `client.files()` → `FilesHandler`

```toml
force = { version = "...", features = ["files"] }
```

## Methods

`upload`, `download`, `link_to_record`. Bodies are capped at 100 MiB
(`ContentVersion`) / 10 MiB (`ContentDocumentLink` response) and held in memory —
this is not a chunked/streaming transfer; see [ADR-004](../../adr/004-feature-gates.md)
for the feature-gate rationale.

```rust
// Upload a new ContentVersion
let content_version_id = client.files()
    .upload("Invoice", "invoice.pdf", file_bytes)
    .await?;

// Download it back
let bytes = client.files().download(&content_version_id).await?;

// Link the resulting ContentDocument to a record (Account, Contact, ...)
client.files()
    .link_to_record(&content_document_id, &account_id)
    .await?;
```

## See also

- Rustdoc: `cargo doc --no-deps --features files --open` → `force::api::files`
