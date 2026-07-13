# ADR-033: Tiered, Env-Gated Live-Contract Test Harness

## Status

Accepted

## Context

The crate ships a mature single-binary live-contract suite at
`crates/force/tests/live_salesforce.rs`. It authenticates against a real org
(via one of several flows), runs `#[ignore]`d smoke tests, and skips cleanly
when no credentials are present. That file has grown large and is frequently
touched by in-flight PRs, so extending it in place is contention-prone.

We want broader live coverage — a full REST CRUD lifecycle, pagination, SOSL,
describe, composite (batch + graph), bulk, tooling, UI, GraphQL — plus smokes
for the specialized surfaces (Data Cloud, Apex REST, Consent, Agentforce Models
and Agent API, CPQ, Account Engagement) and the sibling
`force-marketingcloud` crate. Each specialized surface needs extra
configuration (a provisioned tenant, a deployed Apex endpoint, a configured
agent), so they cannot share a single "creds present?" gate.

## Decision

Add sibling live-test binaries plus a shared harness module, leaving
`live_salesforce.rs` untouched:

- `crates/force/tests/common/mod.rs` — shared harness. Replicates the auth
  contract of `live_salesforce.rs` (same env var names, same priority order,
  same skip idiom): a `LiveAuth` enum implementing `Authenticator`, a
  `load_core_config()` loader, and `create_client()`. It also exposes per-tier
  loaders that each read their own env vars and return `Option`.
- `crates/force/tests/live_core.rs` — core tier: wide coverage of the default
  and common surfaces, gated only on core creds.
- `crates/force/tests/live_special.rs` — specialized surfaces, each gated on
  BOTH core creds AND its own tier env var.
- `crates/force/tests/live_account_engagement.rs` — Pardot v5 smoke (separate
  host, business-unit header).
- `crates/force-marketingcloud/tests/live_marketingcloud.rs` — MC Engagement
  smoke (Installed-Package server-to-server auth).

### Tiers and env vars

| Tier | Binary | Gate env vars |
| --- | --- | --- |
| Core auth | `live_core`, `live_special`, `live_account_engagement` | JWT (`SF_JWT_CLIENT_ID`/`SF_JWT_USERNAME`/`SF_JWT_PRIVATE_KEY_PATH`) → Client Credentials (`SF_CLIENT_ID`/`SF_CLIENT_SECRET`/`SF_TOKEN_URL`) → Username-Password (`SF_UP_*`) → bare token (`SF_ACCESS_TOKEN`+`SF_INSTANCE_URL`) → SF CLI (`SF_TARGET_ORG`) |
| Data Cloud | `live_special` | `SF_DATA_CLOUD=1` (+ optional `SF_DATA_CLOUD_TOKEN_URL`, `SF_DATA_CLOUD_API_VERSION`) |
| Apex REST | `live_special` | `SF_APEX_REST_PATH` |
| Consent | `live_special` | `SF_CONSENT_ACTION` + `SF_CONSENT_IDS` (comma-separated) |
| Models | `live_special` | `SF_MODELS_MODEL` |
| Agent API | `live_special` | `SF_AGENT_ID` |
| CPQ | `live_special` | `SF_CPQ_QUOTE_ID` |
| Account Engagement | `live_account_engagement` | `SF_AE_BUSINESS_UNIT_ID` |
| Marketing Cloud | `live_marketingcloud` | `MC_TENANT_SUBDOMAIN` + `MC_CLIENT_ID` + `MC_CLIENT_SECRET` (+ optional `MC_ACCOUNT_ID`, `MC_SCOPE`, `MC_AUTH_URL`) |
| REST upsert (optional) | `live_core` | `SF_UPSERT_SOBJECT` + `SF_UPSERT_EXT_FIELD` + `SF_UPSERT_EXT_VALUE` |

## Principles

### Skip, don't fail

Every test opens with the same idiom: load the tier's config; if `None`, print a
standardized `SKIP <test>: <reason naming the missing var>` line and return
`Ok(())`. A run with no credentials passes with an all-green, all-skipped result.
This keeps the default `cargo test` hermetic (tests are `#[ignore]`) and makes CI
partial-credential runs meaningful rather than red.

### Safety rules for mutations

- Every created record carries the `force-rs-live-test` prefix
  (`LIVE_TEST_PREFIX`) in its `LastName`/`Name`.
- Cleanup always runs: mutating tests capture the created id, run the body so
  that assertion failures surface as returned errors (via `anyhow::ensure!`,
  never a panic that would skip teardown), then best-effort `delete` before
  propagating any error.
- Only records the test created are ever deleted; no destructive operation runs
  against pre-existing org data. Read-mostly smokes are preferred for surfaces
  that cannot mutate safely (bulk uses a read-only query stream; Account
  Engagement and Marketing Cloud only list/read).

### Feature gating

Each test is `#[cfg(feature = …)]` for its surface so narrower feature builds
still compile. The full matrix runs under `--all-features`.

## How it extends `live_salesforce.rs`

`live_salesforce.rs` remains the canonical auth-flow + error-payload contract
suite and is not modified. The new harness deliberately mirrors its env-var
names and skip idiom so operators configure one set of secrets. Where
`live_salesforce.rs` does strict URL normalization/validation, `common/mod.rs`
uses a trimmed normalizer sufficient for a test harness (the strict validation
is already contract-tested in `live_salesforce.rs`).

## CI

`.github/workflows/live-contract.yml` runs each new binary with
`-- --ignored --test-threads=1` and wires the new env vars from
`secrets`/`vars`. No secret values are hardcoded; unset tiers skip.

## Consequences

- Broader live coverage without touching the contention-prone existing file.
- One credential contract across all live binaries.
- Some duplication of the auth loader between `live_salesforce.rs` and
  `common/mod.rs` (Rust test binaries cannot share a non-published module
  cleanly); accepted as the cost of isolation.
