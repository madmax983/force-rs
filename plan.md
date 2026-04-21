1. **Analyze Sentry's work**
   - Check the `crates/force/src/api/bulk/types.rs` and `crates/force/src/api/bulk/query.rs` changes implemented by Sentry based on my previous journal entries.
   - Sentry successfully added `#[serde(default)]`, the `Null` branch in `types.rs`, and the `loop` in `query.rs` `next()` along with robust tests.
2. **Document Findings as Elenchus**
   - No new tests are condemned. The previously failing tests and missing coverage identified in `.jules/elenchus.md` were addressed properly.
   - Output a verdict confirming everything is acquitted (`🟢 Acquitted` / `⭐ Commended`) as no further missing mutants or bugs were found.
3. **Complete pre commit steps**
   - Complete pre commit steps to ensure proper testing, verification, review, and reflection are done.
4. **Submit with attempt_completion**
   - Submit the resolution with the PR title/description formatted appropriately for Elenchus.
