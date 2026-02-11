# Atlas Journal

**Builder Pattern Removal**
**Tangle:** `IngestJobBuilder` and `DynamicSObjectBuilder` added unnecessary boilerplate and indirection, violating YAGNI.
**Blueprint:** Removed builders, introduced factory methods `create_ingest_job` and `create_upsert_job` on `BulkHandler`, and used direct `DynamicSObject::new` construction.
