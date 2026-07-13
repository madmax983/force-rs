# Live Contract Tests

Integration tests that exercise real Salesforce (and Marketing Cloud) APIs.

All live tests are marked `#[ignore]`, so the default `cargo test` stays fully
hermetic. Live tests **skip cleanly** (green, printing a `SKIP …` line naming the
missing variable) when their credentials are absent — a run with no secrets
passes with every live test skipped.

See [ADR-033](../../../docs/adr/033-live-contract-test-harness.md) for the design
rationale and [ADR-002](../../../docs/adr/002-authentication-strategy.md) for the
auth flows.

## Tiers

| Tier / Binary | Required env vars | Covers | Status |
| --- | --- | --- | --- |
| **Auth flows** — `live_salesforce` | one auth set (see below) | JWT / Client-Credentials / Username-Password authenticate + refresh + failure, REST/tooling/composite/ui/graphql/bulk smokes, error payloads, bulk round-trips, Data Cloud token exchange | live-tested |
| **Core** — `live_core` | core creds (see below) | REST CRUD lifecycle, upsert, query + `query_more` pagination, SOSL, describe (global + object), org limits, composite batch, composite graph, bulk 2.0 query stream, tooling query + `execute_anonymous`, UI object-info + create-defaults, GraphQL | live-tested |
| **Data Cloud** — `live_special` | core + `SF_DATA_CLOUD=1` (opt: `SF_DATA_CLOUD_TOKEN_URL`, `SF_DATA_CLOUD_API_VERSION`) | two-step token exchange + `query_sql` | live-tested |
| **Apex REST** — `live_special` | core + `SF_APEX_REST_PATH` | GET a custom `/services/apexrest/{path}` endpoint | live-tested |
| **Consent** — `live_special` | core + `SF_CONSENT_ACTION` + `SF_CONSENT_IDS` (comma-sep) | `read_consent` | live-tested |
| **Models** — `live_special` | core + `SF_MODELS_MODEL` | `generate_text` on `api.salesforce.com` | live-tested |
| **Agent API** — `live_special` | core + `SF_AGENT_ID` | `start_session_default` + `end_session` (best-effort teardown) | live-tested |
| **CPQ** — `live_special` | core + `SF_CPQ_QUOTE_ID` | `read_quote` | live-tested |
| **SOAP** — `live_soap` | core creds | `get_server_timestamp`, `get_user_info`, describe (global + object), `query` read-only smokes + create→delete `Contact` round-trip (retrieve + `query_typed`) | live-tested |
| **Account Engagement** — `live_account_engagement` | core + `SF_AE_BUSINESS_UNIT_ID` | `query_lists` (Pardot v5, separate host + BU header) | live-tested |
| **Marketing Cloud** — `force-marketingcloud`/`live_marketingcloud` | `MC_TENANT_SUBDOMAIN` + `MC_CLIENT_ID` + `MC_CLIENT_SECRET` (opt: `MC_ACCOUNT_ID`, `MC_SCOPE`, `MC_AUTH_URL`) | Installed-Package auth + `assets().list()` | live-tested |

Optional extra for `live_core` upsert: set `SF_UPSERT_SOBJECT`,
`SF_UPSERT_EXT_FIELD`, `SF_UPSERT_EXT_VALUE` to run the external-id upsert path
(otherwise it skips).

## Core credential resolution

`live_core`, `live_special`, `live_soap`, and `live_account_engagement` share
one loader (`tests/common/mod.rs`) that mirrors `live_salesforce.rs`. It tries,
in order:

1. **JWT Bearer** (feature `jwt`): `SF_JWT_CLIENT_ID`, `SF_JWT_USERNAME`,
   `SF_JWT_PRIVATE_KEY_PATH`, optional `SF_JWT_LOGIN_URL`.
2. **Client Credentials**: `SF_CLIENT_ID`, `SF_CLIENT_SECRET`, `SF_TOKEN_URL`.
3. **Username-Password** (feature `username_password`): `SF_UP_CLIENT_ID`,
   `SF_UP_CLIENT_SECRET`, `SF_UP_USERNAME`, `SF_UP_PASSWORD`,
   `SF_UP_SECURITY_TOKEN`, optional `SF_UP_TOKEN_URL`.
4. **Bare access token**: `SF_ACCESS_TOKEN`, `SF_INSTANCE_URL`.
5. **Salesforce CLI**: `SF_TARGET_ORG` (or the default org via `sf org display`).

Shared: `SF_API_VERSION` (default `v62.0`).

## Safety

- Records created by mutating tests carry the `force-rs-live-test` prefix.
- Cleanup always runs (created id captured; body returns errors instead of
  panicking; best-effort `delete` before propagating).
- Only test-created records are deleted; nothing destructive touches existing
  org data. Bulk/AE/MC tiers are read-only smokes.

## Running

```bash
# Hermetic default run (all live tests are #[ignore], so they don't run)
cargo test -p force

# Core tier (single-threaded so mutations don't interleave)
cargo test -p force --all-features --test live_core -- --ignored --test-threads=1

# Special surfaces
cargo test -p force --all-features --test live_special -- --ignored --test-threads=1

# Account Engagement (Pardot)
cargo test -p force --all-features --test live_account_engagement -- --ignored --test-threads=1

# Marketing Cloud (sibling crate)
cargo test -p force-marketingcloud --test live_marketingcloud -- --ignored --test-threads=1

# See the SKIP lines when credentials are absent
cargo test -p force --all-features --test live_core -- --ignored --test-threads=1 --nocapture
```

CI runs these on a schedule via `.github/workflows/live-contract.yml`.
