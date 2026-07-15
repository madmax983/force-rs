# Choosing an Auth Flow

`force-rs` supports every OAuth 2.0 flow Salesforce offers, plus the divergent
flows used by Data Cloud, Marketing Cloud, and Agentforce. Each authenticator
implements the [`Authenticator`](../adr/002-authentication-strategy.md) trait and
is plugged into the builder via `.authenticate(auth)`. This guide picks the right
flow for a caller and shows a minimal, real builder snippet for each.

All flows except client credentials are feature-gated. Enable the feature in
`Cargo.toml`:

```toml
force = { version = "...", features = ["jwt"] }   # or auth_code, username_password, data_cloud
```

## Decision guide

| Caller / scenario | Flow | Feature flag | Refresh token? |
| --- | --- | --- | --- |
| Server-to-server integration, no user context | Client Credentials | built-in (none) | No — re-auth |
| Backend/daemon with a Connected App X.509 cert | JWT Bearer | `jwt` | No — re-auth |
| Interactive / browser / CLI / native app | Authorization Code + PKCE | `auth_code` | Yes (request `refresh_token` scope) |
| Legacy password integration (org still requires it) | Username-Password | `username_password` | Yes (if org returns one) |
| Data Cloud (CDP) SQL / query | Data Cloud token exchange | `data_cloud` | No — re-exchange |
| Marketing Cloud Engagement REST | Installed Package (client credentials) | `force-marketingcloud` crate | No — re-auth |
| Agentforce Models / Agent API | Client Credentials via an **External Client App** | built-in (`models` / `agent_api` handlers) | No — re-auth |

Rule of thumb: prefer **Client Credentials** for machine-to-machine and **JWT
Bearer** when you hold a certificate and need to act as a specific user.
Username-Password is deprecated by Salesforce — avoid it for new work.

---

## Client credentials (built-in, no feature)

**When to use:** headless server-to-server integrations that act as their own
Connected App user. Simplest flow; no user interaction, no certificate.

**Feature gate:** none — always compiled.

**Credentials:** `client_id`, `client_secret`, and the org token URL (or a My
Domain base URL).

```rust
use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;

let auth = ClientCredentials::new_my_domain(
    client_id,
    client_secret,
    "https://my-org.my.salesforce.com",
);
let client = ForceClientBuilder::new().authenticate(auth).build().await?;
```

Constructors: `ClientCredentials::new(client_id, client_secret, token_url)`,
`new_my_domain(client_id, client_secret, my_domain_url)`,
`new_production(client_id, client_secret)`,
`new_sandbox(client_id, client_secret)`. Override the internal HTTP client with
`.with_client(reqwest_client)`.

**Gotchas:**
- No refresh token is issued. On expiry the `TokenManager` re-runs
  `authenticate()` — same credentials, new token.
- Requires the Connected App to have the *Client Credentials* flow enabled and a
  run-as user assigned.

---

## JWT Bearer (feature: `jwt`)

**When to use:** backends that hold an RSA private key whose certificate is
uploaded to a Connected App. Acts on behalf of a specific `username` with no
interactive login and no stored secret in the request.

**Feature gate:** `jwt`.

**Credentials:** `client_id` (consumer key), Salesforce `username`, and the RSA
private key in PEM form. The audience is the login host.

```rust
use force::auth::JwtBearerFlow;
use force::client::ForceClientBuilder;

let private_key = std::fs::read_to_string("server.key")?;
let auth = JwtBearerFlow::new_production(client_id, "user@example.com", private_key)?;
let client = ForceClientBuilder::new().authenticate(auth).build().await?;
```

Constructors: `JwtBearerFlow::new(client_id, username, private_key_pem, audience,
token_url)` returns `Result` (the PEM is parsed eagerly), plus
`new_production(...)` and `new_sandbox(...)`. `.with_client(...)` overrides the
HTTP client.

**Gotchas:**
- `new*` returns `Result` — an invalid PEM fails at construction with
  `AuthenticationError::InvalidJwtConfig`, not at first request.
- The user must have pre-authorized the Connected App (or admin-approved it) or
  the assertion is rejected.
- No refresh token; expiry triggers a fresh JWT assertion.

---

## Authorization Code + PKCE (feature: `auth_code`)

**When to use:** interactive clients — web apps, CLIs, native/desktop apps —
where a human logs in through a browser. PKCE protects public clients that can't
keep a secret.

**Feature gate:** `auth_code`.

**Credentials:** `client_id`, `redirect_uri`, an optional `client_secret`
(`Some` for confidential clients, `None` for public/PKCE-only), and — after the
browser round-trip — the single-use `code` plus the matching PKCE
`code_verifier`.

The flow is three stages; see the full runnable example at
[`examples/auth_code_pkce.rs`](../../crates/force/examples/auth_code_pkce.rs).

```rust
use force::auth::{AuthorizationCode, AuthorizeUrlBuilder, PkceChallenge};
use force::client::ForceClientBuilder;

// Stage 1: generate PKCE material and the authorize URL to open in a browser.
let pkce = PkceChallenge::generate();
let authorize_url = AuthorizeUrlBuilder::new(&login_url, &client_id, &redirect_uri, &pkce)
    .scope("api")
    .scope("refresh_token")   // ask for a refresh token
    .state("opaque-state")
    .build();

// Stage 2: user logs in; Salesforce redirects to redirect_uri?code=...

// Stage 3: exchange the code (+ verifier) for tokens.
let auth = AuthorizationCode::new(
    client_id,
    client_secret,           // Option<String>: None for public clients
    redirect_uri,
    received_code,
    pkce.verifier(),
    token_url,
);
let client = ForceClientBuilder::new().authenticate(auth).build().await?;
```

`AuthorizeUrlBuilder` also has `new_production(...)` / `new_sandbox(...)` and a
bulk `.scopes(iter)`. `PkceChallenge::generate()` mints a fresh verifier;
`PkceChallenge::from_verifier(v)` reconstructs one when the verifier was
persisted across the HTTP redirect. `AuthorizationCode` mirrors the constructors
(`new`, `new_production`, `new_sandbox`).

Convenience shortcut on the builder — skip constructing `AuthorizationCode`
yourself:

```rust
let client = ForceClientBuilder::new()
    .with_authorization_code(client_id, None, redirect_uri, code, pkce.verifier(), token_url)
    .build()
    .await?;
```

**Token revocation:** `AuthorizationCode::revoke(&self, token)` and
`revoke_stored_refresh_token(&self)` hit `/services/oauth2/revoke`.

**Gotchas:**
- Request the `refresh_token` scope explicitly or you get no refresh token and
  every expiry forces a new interactive login.
- The `code` is single-use and short-lived; exchange it immediately.
- The `code_verifier` passed to the exchange must match the challenge sent to
  `/authorize` — persist it if the callback lands in a different process.

---

## Username-Password (feature: `username_password`)

**When to use:** only for legacy orgs that still mandate it. **Deprecated by
Salesforce.** The feature gate is a deliberate speed bump — prefer JWT Bearer or
Client Credentials for anything new.

**Feature gate:** `username_password`.

**Credentials:** `client_id`, `client_secret`, `username`, `password`, and a
`security_token`. The security token is **automatically appended** to the
password; pass an empty string only when the caller's IP is whitelisted in the
Connected App.

```rust
use force::auth::UsernamePassword;
use force::client::ForceClientBuilder;

let auth = UsernamePassword::new_production(
    client_id,
    client_secret,
    "user@example.com",
    password,
    security_token,   // "" if IP-whitelisted
);
let client = ForceClientBuilder::new().authenticate(auth).build().await?;
```

Constructors: `new(client_id, client_secret, username, password, security_token,
token_url)`, `new_production(...)`, `new_sandbox(...)`.

**Gotchas:**
- The password is exposed to the client application — a core reason Salesforce
  deprecated this flow.
- If the org returns a refresh token it is stored and rotated; on a revoked
  refresh token the authenticator falls back to a full password re-auth. See
  [ADR-025](../adr/025-username-password-auth.md).

---

## Data Cloud token exchange (feature: `data_cloud`)

**When to use:** Data Cloud (CDP) SQL/query access. Data Cloud requires a
**second** token, obtained by exchanging a normal platform token at
`/services/a360/token`.

**Feature gate:** `data_cloud`.

**Credentials:** none of its own — it decorates whatever platform authenticator
you already configured. `DataCloudAuthenticator` wraps the platform
`TokenManager` and performs the two-step exchange; the result is a separate
DC-tenant session managed by its own `TokenManager`.

```rust
use force::auth::{ClientCredentials, DataCloudConfig};
use force::client::ForceClientBuilder;

let client = ForceClientBuilder::new()
    .authenticate(ClientCredentials::new_production(client_id, client_secret))
    .with_data_cloud(DataCloudConfig::default())
    .build()
    .await?;
```

`DataCloudConfig` lets you override the exchange URL (`token_exchange_url`) and
the DC API version (`api_version`); both default from the platform token /
config. See [surfaces/data-cloud.md](surfaces/data-cloud.md) and
[ADR-022](../adr/022-data-cloud-api-design.md).

**Gotchas:**
- `.with_data_cloud(...)` is chained **after** `.authenticate(...)` — it augments
  an already-authenticated builder.
- The DC token targets a distinct tenant host (`*.c360a.salesforce.com`) and
  carries no refresh token; expiry re-runs the exchange against the platform
  token.

---

## Marketing Cloud v2 token (`force-marketingcloud` crate)

**When to use:** Marketing Cloud Engagement REST. This is a **separate sibling
crate** — Marketing Cloud diverges from core Salesforce OAuth (JSON request
body, per-tenant auth subdomain, MID-scoped tenancy).

**Crate:** `force-marketingcloud` (not a feature of `force`).

**Credentials:** an **Installed Package** Server-to-Server API Integration
(`client_id` + `client_secret`), the tenant subdomain, and optionally a default
business unit (MID / `account_id`).

```rust
use force_marketingcloud::MarketingCloudClient;

let client = MarketingCloudClient::builder()
    .tenant_subdomain("mc-tenant-subdomain")
    .client_credentials(client_id, client_secret)
    .account_id("510000000")   // optional default MID
    .build()?;
```

The token endpoint is derived as
`https://{subdomain}.auth.marketingcloudapis.com/v2/token`; `.auth_url(...)`
overrides it for tests. See [surfaces/sibling-crates.md](surfaces/sibling-crates.md).

**Gotchas:**
- Tokens are **short-lived (~20 min, MC reports `expires_in: 1080`) with no
  refresh token** — "refresh" is just re-authentication.
- Tokens are cached **per business unit (MID)**: each `account_id` keeps its own
  token, with single-flight re-auth per key and a 60-second proactive buffer.
- The request body is JSON, not form-encoded — a common surprise if you assume
  standard OAuth.

---

## Agentforce (External Client App required)

**When to use:** Agentforce Models API (LLM gateway) and Agent API (headless
agent sessions), reached via `client.models()` / `client.agents()`.

**Auth:** reuses the standard `ClientCredentials` authenticator — no special
authenticator type. The catch is **registration**: the backing OAuth client must
be a Salesforce **External Client App** (not a classic Connected App) with the
Einstein/Models platform scopes enabled, and the agent must be linked to it.

**Host:** these handlers target the fixed host `https://api.salesforce.com` with
version-less paths (override via `with_host(...)` for Government Cloud
`api.gov.salesforce.com` or testing), not the org `instance_url`.

```rust
use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;

let client = ForceClientBuilder::new()
    .authenticate(ClientCredentials::new_production(client_id, client_secret))
    .build()
    .await?;
let reply = client.models().generate_text(/* ... */).await?;
```

See [surfaces/agentforce.md](surfaces/agentforce.md) and
[ADR-028](../adr/028-agentforce-api-design.md).

**Gotchas:**
- The External Client App requirement is enforced by Salesforce configuration,
  not the type system — misconfiguration surfaces at runtime as an auth error.
- No refresh token; expiry re-authenticates via client credentials.

---

## Token lifecycle & refresh

Every `force` authenticator is wrapped in a `TokenManager` that caches the
`AccessToken` and refreshes on demand. Two refresh paths exist:

- **Proactive (soft) refresh** — `AccessToken::is_soft_expired()` returns true
  within a **60-second buffer** before hard expiry. The `TokenManager` refreshes
  in the background using a non-blocking `try_lock` (`handle_soft_refresh`) so
  readers keep serving the still-valid token.
- **Reactive (401-triggered) refresh** — on a `401` the HTTP middleware calls
  `TokenManager::force_refresh()`, which invalidates the cached token and re-auths
  before retrying the request once. See
  [03-operations.md](03-operations.md) for the middleware retry loop.

Concurrent refreshes are serialized by a `refresh_lock` with double-checked
reads, so a burst of expired-token requests triggers exactly one token fetch.

The `Authenticator` trait has two methods — `authenticate()` and `refresh()`.
Flows that lack a refresh token implement `refresh()` by simply re-running
`authenticate()`:

| Flow | Native refresh token | On expiry |
| --- | --- | --- |
| Client Credentials | No | re-authenticate |
| JWT Bearer | No | new signed assertion |
| Authorization Code + PKCE | Yes (with `refresh_token` scope) | refresh-token grant, rotated + stored |
| Username-Password | Sometimes (org-dependent) | refresh grant, else password re-auth |
| Data Cloud | No | re-run token exchange |
| Marketing Cloud | No (never issued) | re-authenticate (per-MID) |

For flows with refresh tokens, `force-rs` stores the token, uses it on refresh,
rotates to any newly returned refresh token, and **gracefully falls back to a
full re-authentication** if the refresh token has been revoked. Marketing Cloud
has no refresh concept at all — its `TokenManager` only ever re-authenticates.

---

## Secrecy handling

All sensitive credentials are wrapped in the [`secrecy`] crate's `SecretString`
so they are never logged or `Display`ed:

- `ClientCredentials.client_secret`, `UsernamePassword.{client_secret, password,
  security_token}`, and `PkceChallenge.verifier` are all `SecretString`.
- `UsernamePassword` and `PkceChallenge` hand-implement `Debug` to print
  `[REDACTED]` in place of secrets.
- The refresh token stored by `UsernamePassword` is an
  `Arc<RwLock<Option<SecretString>>>`.
- Secrets are only unwrapped via `ExposeSecret::expose_secret()` at the moment a
  token request body is built — never held in plain `String` fields.

Pass raw `&str` / `String` to the constructors; the authenticator wraps them in
`SecretString` internally. Marketing Cloud's `InstalledPackageCredentials` takes
its secret as an explicit `SecretString` argument.

---

## See also

- [ADR-002 — Authentication strategy](../adr/002-authentication-strategy.md)
- [surfaces/data-cloud.md](surfaces/data-cloud.md)
- [surfaces/agentforce.md](surfaces/agentforce.md)
- [surfaces/sibling-crates.md](surfaces/sibling-crates.md)
- [`examples/auth_code_pkce.rs`](../../crates/force/examples/auth_code_pkce.rs)

[`secrecy`]: https://docs.rs/secrecy
