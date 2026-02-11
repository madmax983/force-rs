## [Reduction]
**Bloat:** `DynamicSObjectBuilder`
**Cut:** Removed in favor of `DynamicSObject::new` and `set_field`.
**Saved:** ~50 lines of code / 1 redundant abstraction

## [Reduction]
**Bloat:** `IngestJobBuilder`
**Cut:** Removed in favor of `BulkHandler::create_ingest_job` and `BulkHandler::create_upsert_job`.
**Saved:** ~60 lines of code / 1 redundant abstraction
