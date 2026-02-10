## [Reduction]
**Bloat:** `DynamicSObjectBuilder` - A "Factory Factory" wrapping `DynamicSObject` just to call `set_field`.
**Cut:** Removed builder. Used `DynamicSObject::new` and `set_field` directly.
**Saved:** ~50 LOC / Removed unnecessary redirection.

## [Reduction]
**Bloat:** `IngestJobBuilder` - A builder that just took arguments and called `handler.create_job`.
**Cut:** Replaced with explicit `create_ingest_job` and `create_upsert_job` methods on `BulkHandler`.
**Saved:** ~60 LOC / Improved discoverability and type safety (prevented setting external ID on non-upsert jobs).
