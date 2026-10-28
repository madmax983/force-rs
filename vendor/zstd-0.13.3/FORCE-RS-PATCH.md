# Local patch: force-rs

This is the upstream `zstd` 0.13.3 source (MIT,
<https://github.com/gyscos/zstd-rs>) with the fixes from `zstd` 0.14.3
backported.

## Why a patch

`parquet` 57 and `apache-avro` 0.21 (via `force-lake` and `force-pubsub`)
require `zstd` 0.13. The fixes are in 0.14.3. A lockfile update cannot reach
them.

## Changes from upstream 0.13.3

- `stream/raw.rs`: `Decoder::with_context` and `Encoder::with_context` reset
  the context session first
  ([RUSTSEC-2026-0391](https://rustsec.org/advisories/RUSTSEC-2026-0391)).
  Before, data from an abandoned stream leaked into the next stream on the
  same context.
- `dict.rs`: `from_continuous` adds the sample sizes with overflow detection.

Each change has a `force-rs patch` comment.

**Remove this directory and the `[patch.crates-io]` entry** when the
dependency tree accepts `zstd >= 0.14.3`.
