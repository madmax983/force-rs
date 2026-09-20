# 🔭 Vantage: Spec for Data Seeder — structured failure reporting

> **Status:** the `data_utility` feature shipped `DataSeeder` — it already
> generates mock records via `DataFaker` and inserts them through the
> Composite Batch API, chunking automatically and supporting
> `halt_on_error`; see
> [`docs/guide/surfaces/data-utility.md`](../guide/surfaces/data-utility.md).
> What's below is the part of the original spec that did **not** ship:
> structured per-record failure reporting. `DataSeeder::seed` returns only a
> `usize` count of successful inserts — a failed record is silently dropped
> when `halt_on_error` is `false`, and when it is `true` the whole call
> returns one generic `"Seed operation failed"` error with no indication of
> which record failed or why.

**What business problem does this solve?**
When seeding hundreds of records, some subset commonly fails validation
(a required field the faker didn't populate correctly, a duplicate rule, a
reference field that needs a real ID — see the caveat on
[`data-utility.md`](../guide/surfaces/data-utility.md)). Today a caller has
no way to find out *which* records failed or *why* without re-implementing
the batch loop themselves.

**Gap Analysis:**
`DataSeeder::seed` (`crates/force/src/data/data_seeder.rs`) inspects each
Composite Batch sub-response's status code to decide whether to count a
success, but discards the sub-response body entirely — including the
Salesforce error code/message a failed record would carry.

👤 **User Story:**
As a Salesforce Developer seeding test data, I want `seed()` to tell me
which records failed and what Salesforce said about each one, so that I can
fix bad mock data or the target object's validation rules without turning
on `RUST_LOG=debug` and re-reading raw Composite Batch responses myself.

✅ **Acceptance Criteria:**
- Must return, in addition to (or instead of) the current success count, the
  per-record outcome: index, and — for failures — the Salesforce error
  code(s) and message(s) from that sub-response.
- Must preserve today's `halt_on_error` behavior for callers who want to
  stop at the first failure; the structured report applies to both modes.
- Must not require a second network round-trip to fetch what the Composite
  Batch response already returned.

🚫 **Out of Scope:**
- Automatic retry of failed records.
- Changing `generate_mock_record`'s field-value heuristics (tracked
  separately — see the Reference-field caveat in `data-utility.md`).
