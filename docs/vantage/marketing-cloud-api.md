# 🔭 Vantage: Spec for Marketing Cloud API

## The "So What?"
Marketing Cloud Engagement uses a completely different API stack and auth model than core Salesforce CRM. We need a dedicated way to send transactional emails, trigger journeys, and manipulate assets without polluting the core CRM client, enabling seamless, automated campaign execution from our own backends.

## 👤 User Story
As a Marketing Automation Developer, I want to send transactional emails and trigger customer journeys programmatically, so that I can seamlessly integrate our custom backend with Marketing Cloud Engagement campaigns.

## Metric Definition
- **Success:** P99 request latency for transactional messaging < 50ms (excluding network to API).
- **Scale:** Supports concurrent requests spanning multiple business units without token stampedes.

## Gap Analysis
Current `force` core client assumes a single CRM auth model (OAuth forms, refresh tokens, single instance URL). Marketing Cloud uses per-tenant JSON credentials, dual instance URLs, no refresh tokens, and requires strict per-MID token caching. Existing tools lack a clean, async, typed rust client specifically tailored to these boundaries.

## ✅ Acceptance Criteria
- Must authenticate via JSON client credentials to the tenant-specific auth subdomain.
- Must transparently cache and proactively manage short-lived (20 min) tokens per business unit (MID).
- Must support transactional messaging (email and SMS).
- Must support Content Builder assets (create/get/update/delete/list).
- Must support Data Extension rowset mutations and queries.
- Must support triggering and listing interaction journeys.
- Must include a raw request hatch for unsupported endpoints.

## 🚫 Out of Scope
- SOAP API operations (deferred for future specs).
