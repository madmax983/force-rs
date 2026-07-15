# Release and Versioning

Operator guide for cutting releases across the `force-rs` workspace.

## Workspace versioning

- **Current version**: `0.4.0`, shared across all workspace crates via
  `[workspace.package] version` in the root `Cargo.toml`.
- **Edition**: 2024.
- Each crate inherits `version.workspace = true`, so the workspace releases as a
  single lockstep line — bump the workspace version, and every crate moves
  together.
- The `path` dependencies between siblings pin `version = "0.4.0"` explicitly, so
  a version bump must be reflected in those edges when publishing.

Crates in the workspace:

| Crate | Depends on (siblings) | Standalone |
| --- | --- | --- |
| `force` | — | base crate |
| `force-pubsub` | `force` | no |
| `force-sync` | `force`, `force-pubsub` | no |
| `force-lake` | `force` (`rest`, `bulk`, `schema`) | no |
| `force-marketingcloud` | — | yes (decoupled) |

## Publish order

Publish in dependency order so each crate's sibling dependencies are already on
crates.io:

```
force → force-pubsub → force-sync → force-lake
force-marketingcloud   (independent)
```

Why this order — verified from the crates' `Cargo.toml` dependency edges:

- **`force`** first. It has no sibling dependencies; every other non-standalone
  crate depends on it (`force-pubsub`, `force-sync`, and `force-lake` all declare
  `force = { path = "../force", version = "0.4.0" }`).
- **`force-pubsub`** next. Depends only on `force`.
- **`force-sync`** after both. It declares `force` **and**
  `force-pubsub = { path = "../force-pubsub", version = "0.4.0" }`, so both must
  publish first.
- **`force-lake`** requires `force` (with the `rest`, `bulk`, and `schema`
  features) but does **not** depend on `force-pubsub` or `force-sync`. It only
  needs `force` published; listing it last in the chain is safe because its one
  edge (`force`) is already up.
- **`force-marketingcloud`** has **no** `force` dependency — it is a standalone
  Marketing Cloud Engagement client. It can be published independently of the
  chain, in any order, on its own cadence.

Publish metadata (`description`, `keywords`, `categories`, `documentation`,
`homepage`, `readme`) is set per-crate in each `Cargo.toml` and is
crates.io-ready.

## MSRV policy

- **Workspace MSRV**: Rust **1.92** (`[workspace.package] rust-version = "1.92"`).
- `force` and `force-lake` build at 1.92. `force-lake` is what **raised** the
  workspace floor to 1.92 — its `iceberg` / `iceberg-catalog-s3tables` 0.9 and
  Arrow/Parquet 57 dependencies (S3 Tables support) require it.
- `force-marketingcloud` and `force-sync` individually pin a lower
  `rust-version = "1.85"` in their own `Cargo.toml`, reflecting their lighter
  dependency graphs. Building the **workspace** as a whole still requires 1.92
  because of `force-lake`.

MSRV bumps are treated as a meaningful change:

- Raising the MSRV is done deliberately and only when a dependency or language
  feature requires it (as with the 1.92 raise for iceberg 0.9).
- The MSRV is documented in `Cargo.toml` (`rust-version`) so `cargo` refuses to
  build on older toolchains rather than failing obscurely.
- Treat an MSRV increase as at least a MINOR-level, changelog-worthy event.

## API stability and SemVer

Compatibility for the public `force` API follows
[docs/governance/api-stability-policy.md](../governance/api-stability-policy.md).
Summary:

- **SemVer contract**:
  - `MAJOR` — breaking public API changes.
  - `MINOR` — backward-compatible features and expanded support; new APIs may be
    added.
  - `PATCH` — backward-compatible fixes and docs/test updates.
- **Public API** includes exported Rust types/functions/traits/modules, behavior
  contracts documented in public API docs, **and feature-flag names and their
  meaning**.
- **Stability guarantees**: no breaking change to public API in `PATCH` or
  `MINOR`; behavior-altering bug fixes are documented in the changelog.
- **Feature-flag contract**:
  - Existing stable feature flags are public contract.
  - Renaming or removing a stable flag is a **breaking** change.
  - Experimental flags may change but must be documented as experimental.
  - Cross-feature interactions are tested in CI where practical.
- **Salesforce API versions**: multiple Salesforce API versions are supported in
  one crate line (not one crate per Salesforce version); upgrades require matrix
  tests and live-contract validation.
- **Deprecations**: include migration guidance, span at least one minor release,
  and are removed only in the next major unless security-critical.
- **CI enforcement**:
  - Lint: `cargo clippy --all-targets --all-features -- -D warnings`
  - Test: `cargo test --all-features -- --test-threads=1`
  - Nightly [live-contract workflow](04-live-contract-testing.md) validates the
    Salesforce integration contracts.

## See also

- [Live Contract Testing](04-live-contract-testing.md)
- [ADR-033: Tiered, Env-Gated Live-Contract Test Harness](../adr/033-live-contract-test-harness.md)
- [API Stability and SemVer Policy](../governance/api-stability-policy.md)
