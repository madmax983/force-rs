# Local patch: force-rs

This is the upstream `zstd-safe` 7.3.0 source (MIT/Apache-2.0,
<https://github.com/gyscos/zstd-rs>) with the fix for
[RUSTSEC-2026-0403](https://rustsec.org/advisories/RUSTSEC-2026-0403)
backported from `zstd-safe` 8.2.0.

## Why a patch

The fix is in `zstd-safe` 8.2.0. `zstd` 0.13 requires `zstd-safe` 7.x, and
`parquet` 57 and `apache-avro` 0.21 (via `force-lake` and `force-pubsub`)
require `zstd` 0.13. A lockfile update cannot reach 8.2.0.

## Changes from upstream 7.3.0

- Added `OutBufferWrapper::check()` and `InBufferWrapper::check()`. On error,
  they put `pos` back to the parent's position. Copied from 8.2.0.
- `compress_stream`, `compress_stream2`, `flush_stream`, `end_stream`, and
  `decompress_stream` pass their result through `check()`, as 8.2.0 does.
  Before, a failed call committed the position that zstd reported, which can
  expose uninitialized memory in a `Vec<u8>` output.

Each change has a `force-rs patch (RUSTSEC-2026-0403)` comment.

**Remove this directory and the `[patch.crates-io]` entry** when the
dependency tree accepts `zstd-safe >= 8.2.0`.
