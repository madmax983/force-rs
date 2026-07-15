# Consent & Portability API

GDPR/CCPA consent status checks and data-subject portability (DSAR) exports over the
`/services/data/vXX.X/consent/` and `/portability/` REST endpoints.

- **Feature flag:** `consent`
- **Handler accessor:** `client.consent()` → `ConsentHandler`

Consent *writes* (creating/updating consent records) go through standard sObject CRUD on
`client.rest()`; this handler is read + portability only.

## Consent reads

`ConsentValue` is `Yes` / `No` / `Unknown`. Any unrecognized wire value deserializes to
`Unknown` — treat unknown as denied for compliance safety.

```rust
use force::api::consent::ConsentValue;

// Single action across multiple records.
let result = client.consent()
    .read_consent("email", &["001xx000003GYk1", "001xx000003GYk2"])
    .await?;

for (id, record) in &result {
    if record.proceed.get("email") == Some(&ConsentValue::Yes) {
        println!("{id}: email consent granted");
    }
}

// Multiple actions at once.
let multi = client.consent()
    .read_consent_multi(&["email", "track", "shouldForget"], &["001xx000003GYk1"])
    .await?;
```

`ConsentResponse` is a `HashMap<String, ConsentRecord>` keyed by record ID. Each
`ConsentRecord` has a `result` (`ConsentResult::Success` / `NotFound` / `Error`) and a
`proceed: HashMap<String, ConsentValue>` map of action → value.

## Portability (DSAR export)

Asynchronous: `request_portability` kicks off compilation and returns a `request_id`; poll
it with `check_portability_status` until `PortabilityStatus::Complete`, then read
`download_url`.

```rust
use force::api::consent::{PortabilityRequest, PortabilityStatus};

let req = PortabilityRequest::new("Contact", vec!["003xx000003GYk1".into()]);
let resp = client.consent().request_portability(&req).await?;

let status = client.consent().check_portability_status(&resp.request_id).await?;
if status.status == PortabilityStatus::Complete {
    println!("download: {}", status.download_url.unwrap());
}
```

## Methods

| Method | Purpose |
|---|---|
| `read_consent(action, ids)` | Consent for one action across records |
| `read_consent_multi(actions, ids)` | Consent for several actions at once |
| `request_portability(&req)` | Start a DSAR data-compilation request |
| `check_portability_status(request_id)` | Poll a portability request |

## See also

- [ADR-024 — Consent & Portability API design](../../adr/024-consent-portability-api-design.md)
- Rustdoc: `force::api::consent` (`ConsentHandler`, `ConsentValue`, `PortabilityRequest`, `PortabilityResponse`)
