# ADR-025: Username-Password Authentication Flow

## Status

Accepted

## Context

Many Salesforce orgs still rely on the OAuth 2.0 Resource Owner Password
Credentials flow (username-password) despite Salesforce deprecating it in
favor of JWT Bearer and Client Credentials. A canonical Salesforce client
must support it for backward compatibility.

Unlike Client Credentials and JWT Bearer, the username-password flow:
1. Runs in the context of a specific user (not an integration user)
2. May return a refresh token for session extension
3. Requires a security token appended to the password

## Decision

### Feature-Gated Implementation

```toml
username_password = []
```

The flow is behind a feature flag as a deliberate speed bump. No extra
dependencies are required — the gate signals "this flow is deprecated,
consider alternatives."

### Per-Authenticator Refresh Token Storage

The `UsernamePassword` authenticator stores the refresh token internally
in `Arc<RwLock<Option<String>>>`. This keeps the refresh token as an
authenticator-internal concern without modifying `AccessToken`,
`TokenManager`, or the `Authenticator` trait.

**Refresh lifecycle:**
1. `authenticate()` — password grant → store refresh token from response
2. `refresh()` — use stored refresh token → update on rotation
3. Refresh fails — clear stored token, fall back to `authenticate()`

**Alternative considered:** A `RefreshableAuthenticator<A>` decorator
pattern. Rejected because only 1-2 Salesforce OAuth flows return refresh
tokens (username-password, authorization code). A decorator adds
composition complexity for insufficient reuse benefit.

### Separate Security Token Parameter

The constructor takes `security_token` as a separate parameter and
concatenates it with the password internally. This prevents the common
gotcha where users forget to append the security token and receive
cryptic `INVALID_GRANT` errors.

### Secret Redaction

`UsernamePassword` implements manual `Debug` to redact `client_secret`,
`password`, and `security_token`. The refresh token is stored as a plain
`String` inside `RwLock` — it never leaves the authenticator except in
outbound HTTP requests.

## Consequences

### Positive

- Backward compatibility with legacy orgs
- Feature gate discourages new usage
- Refresh token support extends sessions without re-sending passwords
- Graceful fallback when refresh tokens are revoked

### Negative

- Exposes user passwords to the client application (inherent to the flow)
- Feature gate adds friction for users who need this flow

## Related

- [ADR-002](002-authentication-strategy.md) — Authentication trait design
