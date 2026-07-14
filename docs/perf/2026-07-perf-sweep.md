# Performance Sweep — July 2026

**Date:** 2026-07-13
**Base:** `trunk-dev` (`8d33058`)
**Scope:** Whole workspace (`force`, `force-lake`, `force-pubsub`; siblings surveyed).
**Method:** Behavior-preserving micro-optimizations only. Every existing test
stays green; hot paths are covered by criterion benchmarks with before/after
numbers captured against a pre-optimization baseline.

## Executive summary

The core HTTP / auth path is already well-optimized and was left alone:

- The `reqwest` client is pooled and reused; TLS is `rustls`.
- The Bearer auth header comes from a cached token; token refresh is
  single-flight with **no lock held across the network sleep**.
- Response decode is a direct `serde_json::from_slice` — no
  `Value` round-trip on the common REST path.
- Retry backoff uses `tokio::time::sleep`; bulk pagination reuses a `VecDeque`.
- Feature-gating is clean: a `rest`-only build pulls no heavy deps, and `full`
  pulls no `arrow` / `iceberg` / `tonic` / `prost`.

The real wins are places doing work **twice per response** or **per-record /
per-message**, concentrated in the newer SOAP layer and in the `force-lake` /
`force-pubsub` crates. This PR lands eight such wins; several larger,
architectural opportunities are documented as follow-ups rather than attempted
here.

## Implemented (this PR)

| # | Finding | Location | Impact | Confidence | Effort | Bench delta |
|---|---------|----------|--------|------------|--------|-------------|
| A | `escape_text` returns `Cow<'_, str>` — no alloc when nothing needs escaping | `crates/force/src/api/soap/envelope.rs:19` | Med (per value) | High | S | no-escape **-55%** (51.8→24.1 ns); needs-escape -26% |
| B | `parse_fault` substring short-circuit — skips a full 2nd DOM parse on every successful response | `crates/force/src/api/soap/fault.rs:54` | High (per response) | High | S | **-99.4%** (74.4 µs→447 ns) |
| C | SOAP typed deserialize via borrowing `MapDeserializer` — no `Value` map, no per-field clone | `crates/force/src/api/soap/typed.rs:63` | High (per record) | High | M | **-61.5%** (109.3→45.1 µs / 50×10) |
| D | HTTP executor drops 2 unconditional per-request `String` allocs | `crates/force/src/http/executor.rs:131` | Low (per request) | High | S | not separately benched |
| E | SOAP operation body pre-sizing + `retrieve` field list without `Vec`+`join` | `crates/force/src/api/soap/crud.rs`, `query.rs` | Low | High | S | folded into A/C benches |
| F | `force-lake` Arrow builders pre-sized; `parse_decimal` from `&str` | `crates/force-lake/src/record_batch.rs:57` | Med (per batch) | High | S | **-12.5%** (3.23→2.83 ms / 1000×15) |
| G | `force-pubsub` skips token + gRPC metadata build on schema cache **hit** | `crates/force-pubsub/src/subscriber.rs:80` | Med (per event on hit) | High | S | not separately benched |
| H | `force-pubsub` moves the payload in typed subscribe instead of cloning | `crates/force-pubsub/src/subscriber.rs:333` | Low (per event) | High | S | not separately benched |

All eight are behavior-preserving:

- **A** — `quick_xml::escape::escape` already returns a `Cow`; we stop forcing
  `.into_owned()`. Output bytes are identical. New unit tests assert the
  no-escape path stays `Cow::Borrowed` and the escaping path still escapes
  `< > & ' "`.
- **B** — a `<...:Fault>` element cannot exist unless the substring `"Fault"`
  appears in the body, and `parse_document` only ever yields a fault when that
  element is present. So the guard changes nothing observable; it only avoids
  building a DOM for fault-free responses. A new test asserts a fault-free
  query body yields `None` and a real fault still parses.
- **C** — a custom `FieldDeserializer` yields a borrowed string for present
  fields and null for `xsi:nil` fields, matching the previous
  `serde_json::Value::{String, Null}` bridge (including `Option<_>` → `None`,
  required scalar → error, string → unit enum variant). The existing `*_typed`
  tests are the correctness oracle and remain green.
- **D** — the tracing span copies the `&str` into its own storage, and
  `TelemetryContext::new` only retains owned copies when hooks are registered,
  so the two `.to_string()`s were pure waste. Both borrows are released before
  `request` is mutated/moved.
- **E** — pre-sizing does not change output; the `retrieve` field list is
  written comma-separated directly into the buffer (bytes identical — a new
  test pins the exact string).
- **F** — capacity only affects allocation, not contents; decimal parsing is
  unchanged (only the source `&str` is borrowed instead of cloned).
- **G** — the cache-hit path returns the same schema `get_or_fetch` would have;
  only the wasted token fetch + metadata build on a hit is removed. The miss
  path is untouched.
- **H** — `msg` is already owned; moving `payload` out avoids a deep
  `serde_json::Value` clone. Fields are destructured and reused verbatim.

## Recommended (not in this PR — risk / architecture)

| Finding | Location | Impact | Why deferred |
|---------|----------|--------|--------------|
| Bulk stream fully drained into `Vec<Value>` before batching | `crates/force-lake/src/snapshot.rs:94` | **High** (unbounded memory / OOM on large objects) | Fix is to feed stream chunks incrementally into the `ArrowWriter` — an architectural change to the snapshot pipeline. |
| SmartIngest re-instantiates a `csv::Writer` and re-serializes the header for **every** record | `crates/force/src/api/bulk/smart_ingest.rs:323` | Med-High | Serialize the header once and append headerless rows into a reused buffer; needs careful byte-accounting for the batch-size splitter. |
| Bulk query buffers each page (up to 100 MB) whole before parsing | `crates/force/src/api/bulk/query.rs:251` | Med | Stream the body into a CSV reader; requires a sync-over-async adapter. |
| SOAP DOM parse allocates an owned name per element + clones names in `parse_record`; `text_owned` re-allocates | `crates/force/src/api/soap/parse.rs` | Med | Consume the DOM in the terminal parsers instead of cloning out of it — structural change to the parse layer. |
| Two `get_token_arc()` calls per request (`resolve_url` + `execute`) | `crates/force/src/session.rs:48,140` | Low-Med | Thread one fetched token through both; invasive across the session API. |
| `try_clone` per retry attempt | `crates/force/src/http/executor.rs:171` | Low | Load-bearing for 401-refresh / retry; leave and document. |
| **Correctness follow-up (not perf):** file upload/download bypass the executor middleware → **no 401-refresh / retry**; also full-clone the token and rebuild the Bearer header | `crates/force/src/api/files.rs:99,151,200` | — | Route file transport through `HttpExecutor`. File a ticket — this is a reliability gap, not just a perf one. |
| CPQ ServiceRouter double-serialization (JSON string inside JSON) | `crates/force/src/api/cpq/` | — | Mandated by the wire format; responses are single-parsed. No action. |
| Separate-host handler URL builders use `format!` + per-call trim | `models` / `agent_api` / `account_engagement` | Low | Could normalize the host once and preallocate; optional. |
| Workspace `tokio = { features = ["full"] }` is always-on | `Cargo.toml:26` | Low | Narrowing features is a compile-time / binary-size win but touches every crate. Flag only. |

## Verified-good (no change needed)

- Pooled, reused `reqwest` client — `crates/force/src/http/executor.rs`.
- Cached Bearer auth header from the token manager — `crates/force/src/session.rs`.
- Single-flight token refresh with no lock held across the network await —
  `crates/force/src/auth/token_manager.rs`.
- Direct `from_slice` response decode, no `Value` round-trip — REST handlers.
- `tokio::time::sleep` backoff + `Retry-After` handling — `crates/force/src/http/retry.rs`.
- `VecDeque` reuse across bulk pagination — `crates/force/src/api/bulk/`.
- Clean feature-gating (`rest`-only pulls no heavy deps; `full` pulls no
  arrow/iceberg/tonic/prost) — `crates/force/Cargo.toml`.

## Benchmark methodology + results

Benchmarks use **criterion 0.5** (release profile). Crate-private SOAP
functions are reached through a non-default `bench-internals` feature that
re-exports them under a `#[doc(hidden)]` `bench_hooks` module — normal builds
are unaffected.

Baseline (`before`) was captured on the benchmark-only commit **before** any
optimization was applied; `after` was captured with all changes in place. The
bench sources compile unchanged against both revisions (`escape_text` is
consumed via `.len()`, which works for both a `String` and a `Cow` return; all
other benched signatures are stable).

```bash
# SOAP
cargo bench -p force --features soap,bench-internals --bench soap_bench -- --save-baseline before   # pre-opt
cargo bench -p force --features soap,bench-internals --bench soap_bench -- --baseline before        # post-opt

# force-lake
cargo bench -p force-lake --bench record_batch_bench -- --save-baseline before
cargo bench -p force-lake --bench record_batch_bench -- --baseline before
```

| Benchmark | Before (median) | After (median) | Change |
|-----------|-----------------|----------------|--------|
| `escape_text/no_escape` (A) | 51.801 ns | 24.140 ns | **-55.1%** |
| `escape_text/needs_escape` (A) | 346.87 ns | 289.15 ns | -25.8% |
| `parse_fault/query_success_50` (B) | 74.386 µs | 447.20 ns | **-99.4%** |
| `records_to_typed/50x10` (C) | 109.29 µs | 45.093 µs | **-61.5%** |
| `build_record_batch/1000x15` (F) | 3.2331 ms | 2.8304 ms | **-12.5%** |

Notes on the numbers:

- **A / no-escape** is the common case (Ids, identifiers, plain field values);
  the removed allocation is the whole cost. The `needs_escape` path still
  allocates an owned `Cow`, so its improvement is secondary and closer to
  measurement noise — the headline win is the no-escape fast path.
- **B** is the largest win: on every fault-free SOAP response the old code
  built a full DOM purely to conclude "no fault". The substring guard collapses
  that to a byte scan (~166× on a 50-record body).
- **C** removes a `serde_json::Map` allocation plus a name-clone and a
  value-clone for every field of every record.
- **F** reflects pre-sized Arrow builders (no reallocation/copy while
  appending) plus a borrowed decimal parse.
- **D, E, G, H** are allocation / redundant-work removals not large enough to
  bench in isolation; they are verified behavior-identical by the existing
  `force`, `force-lake`, and `force-pubsub` test suites.
