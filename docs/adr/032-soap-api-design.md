# ADR-032: SOAP Partner API Design

**Date:** 2026-07-13
**Status:** Accepted
**Feature flag:** `soap`

## Context

The classic Salesforce SOAP Partner API is the oldest integration surface, still
required by legacy tooling and for a handful of calls that never received REST
equivalents. It is unusual among the crate's surfaces in several ways:

1. **XML, not JSON.** Requests and responses are SOAP 1.1 envelopes over
   `{instance_url}/services/Soap/u/{version}`. The shared JSON send path
   (`Session::send_request_and_decode`) does not apply.
2. **Untyped ("Partner") WSDL.** Unlike the Enterprise WSDL, the Partner WSDL is
   generic: every field value crosses the wire as a string and records are not
   strongly typed per org. This is what makes a single, codegen-free client
   viable.
3. **Bare version format.** The endpoint uses `67.0`, not the `v67.0` the crate
   stores in `ClientConfig::api_version` and uses for REST.
4. **Faults are HTTP 500.** A whole-call failure — including an expired session
   (`INVALID_SESSION_ID`) — arrives as HTTP 500 with a `<soapenv:Fault>` body,
   not as the HTTP 401 the shared refresh middleware watches for.
5. **`login()` is retiring.** Salesforce is retiring the SOAP `login()` call
   (Release Update; end of support Summer '27; unavailable in API v65.0+ and
   disabled by default in new orgs).

## Decision

### 1. Partner-Style Generic Records; No Per-Org Codegen

The handler models a record as a generic [`SObject`] — an object type name plus
an ordered list of string field name/value pairs, with an explicit
`fields_to_null` list. There is no per-org typed struct generation. This matches
the untyped Partner WSDL, keeps the crate free of a build-time WSDL codegen step,
and mirrors the permissive typing used elsewhere (e.g. `DynamicSObject`).
Response records reuse the same `SObject`; `xsi:nil` fields are stored as `None`.

### 2. `quick-xml` for (De)serialization

XML is handled with `quick-xml`, chosen because:

- It is **already a workspace dependency** (pulled transitively by the `iceberg`
  subtree in `force-lake`), so no new crate family enters the tree.
- It is an **event/streaming parser** with no `serde`-derive codegen and no
  OpenSSL/native dependency, consistent with the crate's `rustls`-only posture.
- It provides the **escaping utilities** (`escape::escape`) needed to safely
  encode caller-supplied text.

Requests are assembled by direct string building with every caller-supplied text
value XML-escaped (see §7). Responses are parsed with an event loop into a small,
namespace-prefix-agnostic DOM keyed on **local** element names, which the
per-call parsers navigate. Matching on local names (and tolerating `soapenv`/`sf`
prefix and element-ordering differences) keeps the parsers robust against the
gap between the canonical Partner-WSDL shape and what a live org emits.

#### RUSTSEC-2026-0194 / RUSTSEC-2026-0195 handling

Both advisories are `quick-xml` DoS issues on untrusted XML, **patched in
0.41.0**. Before this change the workspace declared `quick-xml = "0.37"` (used by
no member) while the `iceberg` subtree independently locked **0.37.5** (via
`reqsign`) and **0.38.4** (via `opendal`) — both vulnerable and both inherited,
unavoidable without upgrading `iceberg`/`opendal`/`reqsign` (breaking, out of
scope).

Decision: bump the workspace pin `quick-xml = "0.37"` → `"0.41"` and consume it
in `force` via `quick-xml = { workspace = true, optional = true }`. This was
verified non-breaking (`cargo check`/`clippy`/tests green), so `force`'s own SOAP
parsing runs on the **patched 0.41.0**. The two pre-existing vulnerable copies in
the `iceberg` subtree are untouched by this change — it introduces **no new
vulnerable copy** and does not vendor a second `quick-xml` for `force`.

### 3. No `login()` — OAuth Token in the `SessionHeader`

The handler does **not** implement SOAP `login()`. It reuses the client's
existing OAuth `TokenManager` and places the access token
(`AccessToken::as_str()`) into `<urn:SessionHeader><urn:sessionId>`. Because the
crate defaults to `v67.0` (> v65.0) `login()` would be unavailable regardless, so
OAuth-in-`SessionHeader` is the only viable path — and it aligns with
Salesforce's own migration guidance. An optional `CallOptions` `client` string
is included by default (`force-rs`) and is overridable via `with_client`.

### 4. Raw-XML Send Path Bypassing the JSON Helper

`Session::send_request_and_decode` is JSON-only, so the handler builds the
request manually on `Session::post`, setting `Content-Type: text/xml;
charset=UTF-8` and `SOAPAction: ""` (the endpoint requires the header but ignores
its value), and dispatches via `Session::execute_request` (auth + retry, no
status gate) — **not** `execute_and_check_success`, which would discard the
fault body on a 500. The response body is read with the capped, DoS-safe
`read_capped_body`, then classified: a `<soapenv:Fault>` becomes an error, any
other non-2xx becomes `HttpError::StatusError`, and a 2xx is parsed as the
operation response.

### 5. `SoapFault` Domain Error vs. Per-Record `SoapError`

A whole-call fault maps to a dedicated `SoapFault { fault_code, fault_string,
exception_code }` wired into `ForceError::Soap` via `#[from]`, mirroring the
feature-gated `ForceError::GraphQL`/`Cpq` variants. Per-record failures
(`SaveResult`/`UpsertResult`/`DeleteResult` `errors`) are **not** faults: they
are successful HTTP 200 responses, so a partially-failed `create` returns
`Ok(Vec<SaveResult>)` with `success = false` on the failed rows. Only transport
faults, `INVALID_SESSION_ID`, and XML parse failures become `Err(ForceError)`.

### 6. Version-Format Stripping

The handler builds `{instance_url}/services/Soap/u/{version}` where `version` is
`ClientConfig::api_version` with any leading `v` stripped (`v67.0` → `67.0`).
Neither `resolve_url` (adds `/services/data/vXX.0`) nor `resolve_apex_rest_url`
fits, so URL construction lives in the handler.

### 7. XML-Escaping of Caller Input

All caller-supplied **text content** — field values, object type names (the text
of `<sf:type>`), SOQL/SOSL strings, ids, and the external-id field name — is
XML-escaped via `quick_xml::escape::escape` before insertion. Field API names are
used as **element names** (`<sf:Name>`) and are treated as trusted Salesforce
identifiers (which are `[A-Za-z0-9_]` plus namespace `__`); they are structural
and cannot be meaningfully "escaped" as an element name. A unit test asserts that
a value containing `<`, `&`, and `"` is correctly escaped.

### 8. `INVALID_SESSION_ID` Manual Refresh-and-Retry

Because an expired session surfaces as an HTTP 500 fault (not a 401), the shared
executor's 401-refresh branch never fires for SOAP. The handler therefore detects
an `INVALID_SESSION_ID` fault (by `exceptionCode`/`faultcode`/`faultstring`),
calls `TokenManager::force_refresh()`, and retries the call **exactly once** —
replicating the executor's single-retry semantics and preserving parity with the
REST 401 path. A second session fault returns `ForceError::Soap`.

### 9. Feature Placement in Both `full` and `all`

`soap` is added to both the `full` and `all` aggregate features. SOAP is a
legacy/completeness surface, but it is part of the standard Salesforce Platform
API umbrella that `full` is meant to represent, so — like `rest`, `bulk`, and the
other first-class surfaces — it belongs in `full` (and therefore transitively in
`all`). Its only extra dependency is `quick-xml`, gated behind `dep:quick-xml`.

## Calls Implemented

`create`, `update`, `upsert`, `delete`, `retrieve`, `query`, `query_more`,
`query_all`, `search`, `describe_sobject`, `describe_sobjects`,
`describe_global`, `get_user_info`, and `get_server_timestamp`.

Typed convenience wrappers: `query_typed`, `query_typed_page`,
`query_more_typed_page`, `retrieve_typed`, `create_typed`, `update_typed`, and
`upsert_typed`.

## Typed Convenience Layer

On top of the generic untyped `SObject` API, `SoapHandler` exposes a serde-typed
convenience layer (`query_typed`, `query_typed_page`, `query_more_typed_page`,
`retrieve_typed`, `create_typed`, `update_typed`, `upsert_typed`). Each method is
a thin adapter that bridges caller structs to and from the `SObject` field bag
via `serde_json` and then delegates to the existing untyped method — the XML and
transport logic is never duplicated.

### Why serde-over-generic (not Enterprise-WSDL codegen)

The Enterprise WSDL yields strongly-typed, per-org sObject definitions, but it
requires a codegen/build step and re-generation whenever the org schema changes.
That directly contradicts this ADR's core decision to ship one generic client
with no build step. Bridging the *caller's own* `#[derive(Serialize,
Deserialize)]` structs against the generic field bag keeps the zero-codegen
property while still giving callers typed ergonomics parity with the REST
`query_typed` path. Callers who want typing opt in per struct; nothing is forced
on the untyped API.

### Stringly-typed field caveat

The Partner API returns **every** field value as a string. Deserialization
therefore assembles a JSON object whose values are all strings (nil fields become
JSON `null`) before handing it to `serde_json`. Target types should model fields
as `String` / `Option<String>`, or supply `#[serde(deserialize_with = "…")]` to
parse a string into another type. On serialization, scalar JSON numbers and
booleans are rendered to their plain string forms (`100`, `true`) to match the
wire; a value that serializes to a nested array/object is rejected with a clear
`ForceError::InvalidInput`, because the flat Partner field bag cannot represent
it. Serialization/deserialization failures surface as
`ForceError::Serialization`.

### Null → `fieldsToNull` mapping

A JSON `null` in a serialized record maps to a `fieldsToNull` entry rather than an
empty field element, matching the Partner API's explicit-null semantics for
`update`/`upsert` (an empty element is silently ignored by Salesforce).

### Pagination choice for `query_typed`

`query_typed` **auto-follows** `queryMore` locators and returns every page's
records combined into a single `Vec<T>`, mirroring `query_all`-style
convenience. For incremental / page-at-a-time consumption, `query_typed_page`
returns `(Vec<T>, done, Option<locator>)` for a single page, and
`query_more_typed_page` continues from a locator — leaving pagination control in
the caller's hands. `retrieve_typed` returns `Vec<Option<T>>` (positional, one
entry per requested Id, `None` for a not-found Id) rather than dropping missing
records, which is more honest than the untyped `retrieve`.

## Consequences

### Positive

- **No WSDL codegen / build step** — one generic client covers every object.
- **Typed ergonomics without codegen** — the serde bridge gives callers typed
  round-trips over the same generic transport, parity with REST `query_typed`.
- **Reuses the OAuth token** — no `login()`, no separate auth flow; forward-
  compatible with the SOAP `login()` retirement.
- **Robust parsing** — local-name, prefix-agnostic DOM tolerates real-world
  envelope variation and the duplicated-`Id` Partner quirk.
- **Session-fault parity** — `INVALID_SESSION_ID` is transparently refreshed and
  retried once, matching REST 401 behavior.
- **`force`'s XML parser is on a patched `quick-xml` (0.41.0)**.

### Negative

- **Nested relationship records / child subqueries are skipped** in the v1 flat
  record model (documented follow-up).
- **`merge`, `convertLead`, `setPassword`, streaming `queryMore` ergonomics, and
  `LimitInfoHeader` surfacing** are follow-ups, not in v1.
- **Inherited `quick-xml` advisories remain** in the `iceberg` subtree (0.37.5 /
  0.38.4); resolving those requires upgrading `iceberg`/`opendal`/`reqsign`,
  which is out of scope for this change.
- **Field/type names are trusted identifiers** — values are fully escaped, but a
  caller passing a syntactically invalid field API name would produce invalid
  XML rather than a validation error.

[`SObject`]: ../../crates/force/src/api/soap/types.rs
