# Winter '27 (API v68.0) Adoption Plan

Status: Implemented. Date: 2026-10-28. ADR: [ADR-035](../adr/035-winter-27-v68-adoption.md).

## 1. Research Summary

Sources: v68.0 developer guide PDFs (REST, Bulk, Tooling, SOQL, UI, Analytics,
SOAP, Streaming) diffed against v67.0. Salesforce KB 005132110, 005461287,
005390912. The `@salesforce/agents@2.3.2` npm source (API Catalog shapes).
Third-party blogs are used only where marked.

| # | Change in Winter '27 | Source | Impact on force-rs |
|---|---|---|---|
| R1 | v68.0 is current. v67.0 is Summer '26 (code says Summer '25). | REST guide | Bump default. Fix labels. |
| R2 | `/services/data/latest/` alias. Versions list adds a `Latest Release` entry. | REST guide | Allow `latest`. Add Versions resource. |
| R3 | v31.0–v40.0 unsupported from Summer '27. | REST guide | None (minimum is v55.0). |
| R4 | Bulk 2.0 `operation`: `refresh`, `consentImport`. `object` is empty for `consentImport`. | Bulk guide | New enum variants. Optional `object`. |
| R5 | Tooling `POST apexCompileResults/` (GA). | Tooling guide | New method. |
| R6 | Tooling `GET symbols/` (Beta). | Tooling guide | New method. |
| R7 | Tooling `GET tests/`: `testLevel` (v68+) replaces `showAllMethods` (v67-). | Tooling guide | New discovery method with version gate. |
| R8 | GraphQL `uiapi.lookupValues` (v68+). | GraphQL guide, release note | New typed helper. |
| R9 | SOQL `FORMULA()` in WHERE (Beta, non-production). | SOQL guide | New builder method. |
| R10 | API Catalog MCP server registry (`/api-catalog/mcp-servers`). | Salesforce CLI source | New surface. |
| R11 | Agent API streaming; per-agent runtime streaming switch. | Agent API docs | Close open follow-up: SSE streaming. |
| R12 | DPoP (RFC 9449) opt-in per app. Proof-less use of a DPoP token revokes it. | Help docs | Fail fast on `token_type: DPoP`. |
| R13 | Username-password flow retires 2027-02-20. Blocked in orgs created Summer '26+. | Release note | Dated deprecation docs. |
| R14 | SOAP `login()`: Use Any API Auth (2026-12-01). Retires Summer '27 (v31–64). | KB 005132110 | None (no `login()`). Fix ADR-032 text. |
| R15 | Refresh-token idle TTL 30 days (prod 2026-11-04). Rotation locked on new ECAs. | KB 005461287, 005390912 | Already rotates and falls back. Document. |
| R16 | Instanced-URL enforcement moved to Spring '27. | Release update | Document: use token `instance_url`. |

## 2. Brainstorming (diverge)

- Bump `ApiVersion::DEFAULT` to v68.0. Add `V68`. Add `release_name()`.
- Accept `"latest"` as `ClientConfig.api_version`.
- `rest().versions()` and `rest().highest_version()`.
- Bulk `JobOperation::Refresh` and `JobOperation::ConsentImport`.
- Tooling `apex_compile_results()`, `symbols()`, `discover_tests()`.
- GraphQL `lookup_values()`.
- SOQL `try_where_formula()`.
- New `api_catalog` feature for the MCP server registry.
- Agent API `send_message_stream()` with an SSE decoder.
- DPoP guard. Full DPoP signer.
- Data Cloud move to Query Connect (`query-sql`).
- Prompt Template generations, Invocable Actions, Testing API, Named
  Credentials, Data Libraries, hosted MCP client.
- Typed error for HTTP 410 (retired version).

## 3. Reverse Brainstorming (how do we break users?)

| How to fail | Counter-measure (AC) |
|---|---|
| Change the default version and break pinned users. | Pinning stays. Default bump is in CHANGELOG. (AC-1) |
| `latest` makes behavior drift without notice. | Opt-in only. Docs say "pin in production". (AC-2) |
| The `Latest Release` entry breaks a versions parser. | Parser keeps the entry and flags it. `highest_version()` skips it. (AC-3) |
| A `consentImport` job sends `object` and the server rejects it. | Empty `object` is not serialized. Client rejects a non-empty one. (AC-4) |
| New `JobOperation` values break job-list decode. | Both values decode. Tests cover them. (AC-4) |
| `showAllMethods` sent on v68 is ignored without notice. | Client returns a validation error. (AC-7) |
| Symbols API is called in parallel and fails. | Doc states the one-request-per-org limit. (AC-6) |
| An SSE stream holds an unbounded buffer. | Decoder caps event size. Test proves it. (AC-11) |
| Unknown SSE event names crash the stream. | Unknown events pass through as `Other`. (AC-11) |
| A DPoP token is sent as `Bearer`/`DPoP` without proof and gets revoked. | Header build fails before send. (AC-12) |
| SOQL `FORMULA()` gets injection through field names. | Field names are validated. Literals are typed numbers. (AC-9) |
| Docs stay stale ("Summer '25"). | Labels come from `release_name()`. Docs updated. (AC-1, AC-14) |
| A new feature is missing from docs tables, and CI drift checks fail. | Update feature matrix and surfaces table. (AC-10, AC-14) |

## 4. Six Thinking Hats

- **White (facts).** Facts come from v68.0 guides (table R1–R16). Data Cloud
  `query-sql` field names are not verified. Agent SSE `event:` names are not
  verified. GraphQL `lookupValues` selection set is partly verified.
- **Red (feelings).** Users want "it just works on v68". Breaking changes cause
  frustration. Keep changes additive where possible.
- **Black (risks).** Unverified wire shapes cause runtime decode failures.
  Mitigation: permissive types (`Option`, `#[serde(default)]`, `extra` maps),
  and `extra` maps for unknown fields. Full DPoP needs new crypto deps and changes every
  request path: high risk in one PR.
- **Yellow (benefits).** v68 default gives FORMULA(), `latest`, and new Tooling
  endpoints. Streaming closes a roadmap gap. API Catalog gives agent-tool
  registry access from Rust.
- **Green (ideas).** `ApiVersion::release_name()` makes labels testable.
  Shared SSE decoder can serve later streaming APIs. Version gating helper
  (`ApiVersion::config_major`) serves future version-gated params.
- **Blue (process).** RED → GREEN → REFACTOR per work item. Then multi-angle
  agent review. Then the AC evidence table. Out-of-scope items go to ADR-035
  as follow-ups.

## 5. Scope Decision

In scope: W1–W10 below. Out of scope (follow-ups in ADR-035): full DPoP
signer, Data Cloud Query Connect migration (wire shape not verified),
Prompt Template generations, Invocable Actions, Agentforce Testing API, Named
Credentials, Data Libraries, hosted MCP client, typed HTTP 410 error. These
are not Winter '27 changes or are not verifiable today.

| Work item | Description |
|---|---|
| W1 | `ApiVersion::V68`, default/tested = v68, `release_name()`, labels fixed, `latest` alias. |
| W2 | REST Versions resource: `versions()`, `highest_version()`. |
| W3 | Bulk `Refresh` + `ConsentImport`, operation-aware validation. |
| W4 | Tooling `apex_compile_results()`, `symbols()`, `discover_tests()` (+ gate). |
| W5 | GraphQL `lookup_values()`. |
| W6 | `api_catalog` feature: MCP server registry handler. |
| W7 | Agent API SSE streaming. |
| W8 | DPoP fail-fast guard. |
| W9 | SOQL `FORMULA()` builder. |
| W10 | Docs: ADR-035, CHANGELOG, guides, CLAUDE.md, auth-flow dates, ADR-032 fix. |

## 6. Acceptance Criteria

| AC | Criterion |
|---|---|
| AC-1 | `ApiVersion::DEFAULT == MAX_TESTED == V68`. `V68` exists. `release_name()` maps v55–v68 correctly. Doc labels fixed. |
| AC-2 | `ClientConfig { api_version: "latest" }` builds `/services/data/latest/...` URLs. |
| AC-3 | `rest().versions()` parses the list with the `Latest Release` entry. `highest_version()` returns the highest numeric version. |
| AC-4 | `JobOperation::{Refresh, ConsentImport}` round-trip as `refresh`/`consentImport`. `consentImport` omits `object`, rejects a non-empty `object` and an external ID. `JobInfo` decodes with no `object`. |
| AC-5 | `tooling().apex_compile_results()` sends `{}` and decodes `OK` and `PARTIAL_FAILURE`. |
| AC-6 | `tooling().symbols()` sends `category` (+ optional `namespace`, `name`; empty namespace kept) and decodes `typeStubs`. |
| AC-7 | `tooling().discover_tests()` sends `X-Chatter-Entity-Encoding: false` and its query params. `testLevel` on v67 or below is an error. `showAllMethods` on v68 or above (or `latest`) is an error. Next page works. |
| AC-8 | `graphql().lookup_values()` sends the documented signature with variables and decodes the result. |
| AC-9 | `SoqlQueryBuilder::try_where_formula()` emits `FORMULA('A - B') > 10`. Invalid field names and non-finite numbers are rejected. |
| AC-10 | `api_catalog` feature: list/create/get/update/delete/fetch/list assets/replace assets on `/api-catalog/mcp-servers`. In `full`. Docs tables updated. |
| AC-11 | `agents().send_message_stream()` posts to `.../messages/stream` with `Accept: text/event-stream` and yields typed events. Decoder handles split chunks, CRLF, comments, multi-line data, unknown events, and caps event size. |
| AC-12 | A token with `token_type` `DPoP` (any case) fails at header build with a clear error. No request is sent. |
| AC-13 | Username-password docs carry the 2027-02-20 retirement date. ADR-032 SOAP `login()` text corrected. Auth guide notes refresh-token idle TTL, rotation, and ECA. |
| AC-14 | ADR-035, CHANGELOG `[Unreleased]`, CLAUDE.md roadmap, operations guide, surfaces docs updated. CI drift scripts pass. |
| AC-15 | `cargo fmt`, `cargo clippy --workspace --all-features --all-targets -D warnings`, and `cargo test --workspace --all-features` pass. Each new behavior has a happy-path, an edge-case, and (where a bug was found) a regression test. |
