# Live Contract Testing

Operator guide for provisioning and running the tiered live-contract test suite
against real Salesforce, Pardot, and Marketing Cloud tenants.

See [ADR-033](../adr/033-live-contract-test-harness.md) for the design rationale
and safety guarantees, and the harness README at
`crates/force/tests/README.md`.

## Overview

Every live test is marked `#[ignore]`, so the default `cargo test` stays fully
hermetic — no network, no credentials required. Live tests are opt-in via
`-- --ignored`.

Live tests **skip cleanly**: when a tier's credentials are absent, the test
prints a standardized `SKIP <test>: <missing var>` line, returns `Ok(())`, and
counts as green. A run with no secrets passes with every live test skipped, so
partial-credential CI runs are meaningful rather than red.

Each test is also `#[cfg(feature = …)]` for its surface; the full matrix runs
under `--all-features`.

## Binaries

| Binary | Crate | Covers |
| --- | --- | --- |
| `live_salesforce` | `force` | Auth flows (JWT / Client-Credentials / Username-Password authenticate + refresh + failure), REST/tooling/composite/ui/graphql/bulk smokes, error payloads, bulk round-trips, Data Cloud token exchange |
| `live_core` | `force` | REST CRUD lifecycle, upsert, query + `query_more` pagination, SOSL, describe (global + object), org limits, composite batch, composite graph, bulk 2.0 query stream, tooling query + `execute_anonymous`, UI object-info + create-defaults, GraphQL |
| `live_special` | `force` | Specialized surfaces (Data Cloud, Apex REST, Consent, Models, Agent API, CPQ), each gated on core creds **and** its own tier var |
| `live_account_engagement` | `force` | Account Engagement (Pardot) v5 `query_lists` (separate host + BU header) |
| `live_marketingcloud` | `force-marketingcloud` | Marketing Cloud Engagement Installed-Package auth + `assets().list()` |

## Env-var tiers

### `live_salesforce` and `live_core` (core org)

Both need one core credential set (see [Credential resolution](#credential-resolution-order)
below) plus the shared config var.

| Env var | Required | Notes |
| --- | --- | --- |
| One core auth set | yes | JWT, Client-Credentials, Username-Password, bare token, or SF CLI |
| `SF_API_VERSION` | no | Shared default `v62.0` |

Optional `live_core` external-id upsert path (skips when any is unset):

| Env var | Required | Notes |
| --- | --- | --- |
| `SF_UPSERT_SOBJECT` | no | SObject for external-id upsert |
| `SF_UPSERT_EXT_FIELD` | no | External-id field API name |
| `SF_UPSERT_EXT_VALUE` | no | External-id value |

### `live_special` (specialized surfaces)

Each surface requires the core creds **and** its own gate var. Surfaces whose
gate var is unset skip individually.

| Surface | Required | Optional |
| --- | --- | --- |
| Data Cloud | `SF_DATA_CLOUD=1` | `SF_DATA_CLOUD_TOKEN_URL`, `SF_DATA_CLOUD_API_VERSION` |
| Apex REST | `SF_APEX_REST_PATH` | — |
| Consent | `SF_CONSENT_ACTION`, `SF_CONSENT_IDS` (comma-separated) | — |
| Models | `SF_MODELS_MODEL` | — |
| Agent API | `SF_AGENT_ID` | — |
| CPQ | `SF_CPQ_QUOTE_ID` | — |

### `live_account_engagement` (Pardot v5)

| Env var | Required | Notes |
| --- | --- | --- |
| Core auth set | yes | Same loader as `live_core` |
| `SF_AE_BUSINESS_UNIT_ID` | yes | Sent as the `Pardot-Business-Unit-Id` header |

### `live_marketingcloud` (`force-marketingcloud`)

| Env var | Required | Notes |
| --- | --- | --- |
| `MC_TENANT_SUBDOMAIN` | yes | Tenant subdomain (TSSD) for the auth/REST hosts |
| `MC_CLIENT_ID` | yes | Installed-Package client id |
| `MC_CLIENT_SECRET` | yes | Installed-Package client secret |
| `MC_ACCOUNT_ID` | no | Business unit MID |
| `MC_SCOPE` | no | Space-separated scope override |
| `MC_AUTH_URL` | no | Auth endpoint override |

## Credential resolution order

`live_core`, `live_special`, and `live_account_engagement` share one loader
(`crates/force/tests/common/mod.rs`) that mirrors `live_salesforce.rs`. It tries
each mechanism in order and uses the first fully-configured one:

1. **JWT Bearer** (feature `jwt`) — `SF_JWT_CLIENT_ID`, `SF_JWT_USERNAME`,
   `SF_JWT_PRIVATE_KEY_PATH`, optional `SF_JWT_LOGIN_URL`.
2. **Client Credentials** — `SF_CLIENT_ID`, `SF_CLIENT_SECRET`, `SF_TOKEN_URL`.
3. **Username-Password** (feature `username_password`) — `SF_UP_CLIENT_ID`,
   `SF_UP_CLIENT_SECRET`, `SF_UP_USERNAME`, `SF_UP_PASSWORD`,
   `SF_UP_SECURITY_TOKEN`, optional `SF_UP_TOKEN_URL`.
4. **Bare access token** — `SF_ACCESS_TOKEN`, `SF_INSTANCE_URL`.
5. **Salesforce CLI** — `SF_TARGET_ORG` (or the default org via `sf org display`).

Shared across all mechanisms: `SF_API_VERSION` (default `v62.0`).

## Provisioning

### Salesforce org / Connected App (JWT — recommended)

JWT Bearer is the primary, renewable flow used by the nightly workflow.

1. Create (or reuse) a Connected App in the target org with OAuth enabled.
2. Generate an X.509 cert / RSA key pair; upload the certificate to the
   Connected App's "Use digital signatures" setting. Keep the private key `.pem`.
3. Enable the OAuth scopes your tests exercise (at minimum `api`; add `refresh_token`
   for refresh coverage). Set the app to admin-pre-authorized and assign the
   running user's profile/permission set.
4. Export:
   - `SF_JWT_CLIENT_ID` — the Connected App consumer key.
   - `SF_JWT_USERNAME` — the integration user to impersonate.
   - `SF_JWT_PRIVATE_KEY_PATH` — path to the private key `.pem`.
   - `SF_JWT_LOGIN_URL` — `https://login.salesforce.com` (prod) or
     `https://test.salesforce.com` (sandbox). Accepts a bare host, base URL, or
     full `/services/oauth2/token` endpoint.

### Salesforce org / Connected App (Client Credentials)

For server-to-server without a subject user.

1. In the Connected App, enable the **Client Credentials Flow** and assign a
   run-as integration user.
2. Export `SF_CLIENT_ID`, `SF_CLIENT_SECRET`, and `SF_TOKEN_URL` (the org base URL
   or full `/services/oauth2/token` endpoint for the target org).

### Specialized surfaces

Set the core creds plus the surface's gate var(s):

- **Data Cloud** — provision a Data Cloud-enabled org; set `SF_DATA_CLOUD=1`.
  Override the token exchange host with `SF_DATA_CLOUD_TOKEN_URL` if needed.
- **Apex REST** — deploy a custom `@RestResource` class; set `SF_APEX_REST_PATH`
  to its `/services/apexrest/{path}` suffix.
- **Consent** — set `SF_CONSENT_ACTION` (e.g. an action name) and
  `SF_CONSENT_IDS` (comma-separated record ids to check).
- **Models** — set `SF_MODELS_MODEL` to a provisioned Agentforce model name
  (runs `generate_text` on `api.salesforce.com`).
- **Agent API** — set `SF_AGENT_ID` to a configured headless agent id.
- **CPQ** — set `SF_CPQ_QUOTE_ID` to an existing `SBQQ__Quote__c` id.

### Pardot business unit (Account Engagement)

1. Ensure Account Engagement (Pardot) is provisioned and the running user has
   access to the target business unit.
2. Grant the `pardot_api` OAuth scope on the Connected App used for core auth.
3. Find the business unit id (Setup → Account Engagement → Business Unit Setup;
   a `0Uv…` id).
4. Export `SF_AE_BUSINESS_UNIT_ID`. It is sent verbatim as the
   `Pardot-Business-Unit-Id` header against the `pi.pardot.com` host.

### Marketing Cloud Installed Package

1. In Marketing Cloud, create an **Installed Package** with an API Integration
   component (server-to-server).
2. Grant the read scopes the smoke needs (Content Builder / assets read).
3. Collect the Installed Package `Client Id`, `Client Secret`, the tenant
   subdomain (TSSD, from the auth base URI), and optionally the target business
   unit MID.
4. Export `MC_CLIENT_ID`, `MC_CLIENT_SECRET`, `MC_TENANT_SUBDOMAIN`, and
   optionally `MC_ACCOUNT_ID` (MID), `MC_SCOPE`, `MC_AUTH_URL`.

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

Run live binaries with `--test-threads=1` so mutating tests do not interleave.

## Nightly workflow

`.github/workflows/live-contract.yml` runs the suite on a schedule and on demand:

- **Triggers**: `schedule` cron `0 4 * * *` (04:00 UTC nightly) and
  `workflow_dispatch` (manual).
- **Runner**: `ubuntu-latest`, 30-minute timeout, `dtolnay/rust-toolchain@stable`
  with `Swatinem/rust-cache`.
- **JWT key handling**: when `SF_JWT_CLIENT_ID` is set, the workflow writes the
  `SF_JWT_PRIVATE_KEY` secret to `/tmp/jwt_live_key.pem` (mode 600), exports
  `SF_JWT_PRIVATE_KEY_PATH`, and always removes the key file in a final cleanup
  step.
- **Steps** (each `-- --ignored --test-threads=1`):
  1. `force` `--test live_salesforce`
  2. `force` `--test live_core`
  3. `force` `--test live_special`
  4. `force` `--test live_account_engagement`
  5. `force-marketingcloud` `--test live_marketingcloud`
  6. `force-pubsub` `--test live_salesforce_pubsub`
- **Config**: `SF_API_VERSION` defaults to `v62.0`; all secrets/vars are wired
  from `secrets`/`vars`. No secret values are hardcoded; unset tiers skip.

## Safety guarantees

- **Prefixed records** — every record created by a mutating test carries the
  `force-rs-live-test` prefix (`LIVE_TEST_PREFIX`) in its `LastName`/`Name`.
- **Best-effort cleanup always runs** — mutating tests capture the created id,
  run the body so assertion failures surface as returned errors (via
  `anyhow::ensure!`, never a panic that would skip teardown), then best-effort
  `delete` before propagating any error.
- **Only test-created records are deleted** — no destructive operation touches
  pre-existing org data.
- **Read-only smokes** for surfaces that cannot mutate safely: bulk uses a
  read-only query stream, and Account Engagement and Marketing Cloud only
  list/read.

## See also

- [ADR-033: Tiered, Env-Gated Live-Contract Test Harness](../adr/033-live-contract-test-harness.md)
- [API Stability and SemVer Policy](../governance/api-stability-policy.md)
- Harness README: `crates/force/tests/README.md`
