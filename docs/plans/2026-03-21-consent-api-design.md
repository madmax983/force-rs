# Consent & Portability API Design

## Overview

Single `consent` feature flag providing `ConsentHandler<A>` with 5 methods
covering GDPR/CCPA consent reads and data portability requests.

## Design Decisions

1. **Single feature**: `consent = []` — consent reads + portability bundled
2. **No consent writes**: Standard sObject CRUD via `.rest()` already handles this
3. **Typed ConsentValue enum**: `Yes`, `No`, `Unknown` — fail-safe for compliance
4. **Raw portability endpoints**: No built-in polling — caller decides strategy
5. **Standard URL resolution**: Uses `Session::resolve_url("consent/...")` — no custom resolver

## Module Layout

```
crates/force/src/api/consent/
├── mod.rs           # ConsentHandler struct + resolve helper
├── types.rs         # ConsentValue, ConsentResult, ConsentRecord, PortabilityRequest/Response/Status
├── action.rs        # read_consent, read_consent_multi
└── portability.rs   # request_portability, check_portability_status
```

## Public API

```rust
// Consent reads
client.consent().read_consent("email", &["001xx...", "003xx..."]).await?;
client.consent().read_consent_multi(&["email", "track"], &["001xx..."]).await?;

// Portability
let resp = client.consent().request_portability(&req).await?;
let status = client.consent().check_portability_status(&resp.request_id).await?;
```

## Types

- `ConsentValue` enum: Yes, No, Unknown (unknown strings → Unknown for safety)
- `ConsentResult` enum: Success, NotFound, Error
- `ConsentRecord`: result + proceed map + extras
- `ConsentResponse`: HashMap<String, ConsentRecord> keyed by record ID
- `PortabilityRequest`: object_type, record_ids + extras
- `PortabilityStatus` enum: Pending, Complete, Failed
- `PortabilityResponse`: request_id, status, download_url + extras

## Endpoints

| Method | HTTP | Path |
|--------|------|------|
| read_consent | GET | /consent/action/{action}?ids=... |
| read_consent_multi | GET | /consent/multiaction?actions=...&ids=... |
| request_portability | POST | /portability |
| check_portability_status | GET | /portability/{request_id} |

## Error Handling

No custom error type — standard Salesforce API errors via ForceError::Api.

## Testing

~15-20 wiremock tests covering success, mixed results, not-found, fail-safe
deserialization, portability lifecycle, and error responses.
