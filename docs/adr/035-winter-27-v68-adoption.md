# ADR-035: Winter '27 (API v68.0) Adoption

## Status

Accepted

## Context

Salesforce released Winter '27 (API v68.0) in October 2026. The research and
the full plan are in
[docs/plans/2026-10-28-winter-27-v68-adoption.md](../plans/2026-10-28-winter-27-v68-adoption.md).
The changes that affect `force-rs`:

1. v68.0 is the current version. The `/services/data/latest/` alias resolves to
   the newest version. The Versions resource lists the alias.
2. Bulk API 2.0 adds the `refresh` and `consentImport` operations.
   `consentImport` jobs have no `object`.
3. Tooling API adds `apexCompileResults` (GA) and `symbols` (Beta). Test
   Discovery replaces `showAllMethods` with `testLevel`.
4. GraphQL adds `uiapi.lookupValues`. SOQL adds `FORMULA()` in WHERE (Beta).
5. The API Catalog (`/api-catalog/mcp-servers`) registers external MCP servers.
6. The Agent API streams replies as Server-Sent Events.
7. An app can require DPoP. A request without a DPoP proof revokes the token.
8. OAuth: refresh tokens expire after 30 idle days; new External Client Apps
   rotate refresh tokens; the username-password flow retires 2027-02-20.

## Decision

1. **Default to v68.0.** `ApiVersion::DEFAULT` and `MAX_TESTED` are `V68`.
   `ApiVersion::release_name()` computes labels from the three-release cycle.
   This also fixes the old doc labels, which were one year late.
2. **`latest` is opt-in and REST-only.** `ClientConfig.api_version` accepts
   `ApiVersion::LATEST_ALIAS`. It is not the default, because behavior can
   change without a code change. Version gates treat `latest` as v68.0+. The
   SOAP handler rejects it, because only the REST guide documents the alias.
3. **Version gates run on the client for parameter conflicts only.** A
   parameter that the configured version does not accept (for example
   `showAllMethods` on v68.0) is an `InvalidInput` error. No request is sent.
   A silent ignore by the server is worse than an early error. New v68-only
   resources have no client gate; the server returns its own error. An
   unknown version string skips the gate.
4. **New `JobOperation` values.** `JobOperation` gets `Refresh` and
   `ConsentImport`. This is breaking for a `match` without a `_` arm (the
   project does not use `#[non_exhaustive]`). `CreateJobRequest::validate`
   holds the operation rules. An empty `object` is not serialized.
5. **API Catalog is its own feature** (`api_catalog`, in `full`), with its own
   handler. Wire shapes come from the Salesforce CLI source. Server enums have
   an `Unknown` fallback.
6. **Agent streaming uses a crate-private SSE decoder.** It follows the
   WHATWG rules: LF, CRLF, and CR; BOM; comments; multi-line data; the last
   event ID persists; an event with no blank line after it is discarded. It
   limits each event to 1 MiB (`HttpError::PayloadTooLarge`). The event kind
   comes from `message.type`, else from the SSE `event:` name; case and `_`
   are ignored. The stream has its own time limit, because the client
   `timeout` covers the full body. A per-request timeout now also sets the
   executor limit. Agent and session IDs must be `[A-Za-z0-9_-]`, so a
   caller value cannot change the endpoint.
7. **DPoP fails fast.** `TokenManager` rejects a token with
   `token_type: DPoP` (`AuthenticationError::UnsupportedTokenType`) before it
   stores it. This covers every consumer: REST, SOAP, the Data Cloud
   exchange, and Pub/Sub. `AccessToken::auth_header` also rejects it. No
   request is sent, so the token is not revoked.
8. **GraphQL `operationName`.** `GraphqlRequest` now sends `operationName`.
   It sent `operation_name` before, and the server ignored it.

## Consequences

- Positive: v68.0 features work by default. Users get early errors for
  version mistakes and DPoP apps.
- Negative: code that matches on `JobOperation` or `AuthenticationError`
  without a wildcard must add the new variants. Tests that mock `/services/data/v67.0/` paths with the
  default config must change to v68.0.
- Wire shapes of `lookupValues` items and Agent SSE `event:` names are not
  fully documented. The types are permissive and keep unknown fields.

## Follow-ups (out of scope)

- Full DPoP support (proof signing, nonce retry).
- Data Cloud Query Connect API (`ssot/query-sql` with status and row paging).
  `query_sql` still uses the legacy `ssot/query` path.
- Prompt Template generations, Invocable Actions, Agentforce Testing API,
  Named Credentials, Agent Data Libraries, hosted MCP client.
- A typed error for HTTP 410 (retired API version). v31.0–v40.0 become
  unsupported in Summer '27.
