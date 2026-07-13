# force-rs Operator's Guide

Task-oriented documentation for running `force-rs` in production: installing and
feature-gating the crate, selecting an auth flow, using each API surface, and
operating, testing, and releasing against live Salesforce orgs. For architectural
rationale see the [architecture overview](../../CLAUDE.md) and the
[ADR index](../adr/README.md).

## Contents

- [01 — Getting Started](01-getting-started.md) — install, feature-flag matrix, MSRV, TLS/rustls, quick start.
- [02 — Choosing an Auth Flow](02-choosing-an-auth-flow.md) — client credentials, JWT, auth code + PKCE, username-password.

### API Surfaces

- [Surfaces overview](surfaces/README.md)
  - [REST](surfaces/rest.md)
  - [Tooling](surfaces/tooling.md)
  - [Bulk](surfaces/bulk.md)
  - [Composite](surfaces/composite.md)
  - [UI](surfaces/ui.md)
  - [GraphQL](surfaces/graphql.md)
  - [Data Cloud](surfaces/data-cloud.md)
  - [Apex REST](surfaces/apex-rest.md)
  - [CPQ](surfaces/cpq.md)
  - [Consent](surfaces/consent.md)
  - [Analytics](surfaces/analytics.md)
  - [Account Engagement](surfaces/account-engagement.md)
  - [SOAP](surfaces/soap.md)
  - [Agentforce](surfaces/agentforce.md)
  - [Sibling crates](surfaces/sibling-crates.md)

### Operations

- [03 — Operations](03-operations.md) — retries, rate limits, token refresh, observability.
- [04 — Live Contract Testing](04-live-contract-testing.md) — validating against real orgs.
- [05 — Release and Versioning](05-release-and-versioning.md) — semver, feature stability, publishing.

## See also

- [Architecture overview](../../CLAUDE.md)
- [ADR index](../adr/README.md)
- [Runbooks](../runbooks/README.md)
