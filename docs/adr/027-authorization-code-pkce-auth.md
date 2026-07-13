# ADR-027: OAuth 2.0 Authorization Code Flow with PKCE

## Status

Accepted

## Context

Salesforce recommends the OAuth 2.0 Authorization Code flow, extended with
Proof Key for Code Exchange (PKCE, [RFC 7636]), as the replacement for the
deprecated user-agent and username-password flows for **interactive**
(browser-based) clients — web apps, single-page apps, and native/mobile apps.

Unlike the flows already supported by `force-rs` (Client Credentials, JWT
Bearer, Username-Password), the Authorization Code flow is **not headless**:
it requires a human to authenticate through a browser and consent to the
requested scopes. This shapes the entire design.

PKCE hardens the flow against authorization-code interception:

1. The client generates a high-entropy `code_verifier` (43–128 characters from
   the unreserved set `[A-Za-z0-9-._~]`).
2. It derives a `code_challenge` = `base64url(SHA-256(code_verifier))` with
   padding removed, and sends it (with `code_challenge_method=S256`) on the
   authorization request.
3. When exchanging the returned `code` for tokens, the client presents the
   original `code_verifier`. Salesforce re-derives the challenge and rejects the
   exchange if it does not match — so a stolen authorization code is useless
   without the verifier.

Salesforce specifics:

- Authorize endpoint: `GET {login}/services/oauth2/authorize` with
  `response_type=code`, `client_id`, `redirect_uri`, space-separated `scope`,
  `state`, `code_challenge`, `code_challenge_method=S256`.
- Token endpoint: `POST {login}/services/oauth2/token` with
  `grant_type=authorization_code`, `code`, `client_id`, `redirect_uri`,
  `code_verifier`, and `client_secret` **only** for confidential clients
  (public/PKCE clients omit it).
- Refresh: `grant_type=refresh_token`, `refresh_token`, `client_id`
  (+ `client_secret` for confidential clients).
- Revocation: `POST {login}/services/oauth2/revoke` with `token=<token>`.
- Salesforce only supports the `S256` challenge method (never `plain`).

## Decision

### Feature-Gated Implementation

```toml
auth_code = ["dep:sha2", "dep:getrandom"]
```

The flow lives behind the `auth_code` feature gate (added to the `full` and
`all` aggregates, alongside `jwt`). It pulls in two small, feature-gated
dependencies:

- `sha2` — SHA-256 for the PKCE challenge derivation (no existing dependency
  provides SHA-256; the codebase's `blake3` is the wrong algorithm).
- `getrandom` — OS-backed CSPRNG for the verifier (already present transitively
  via `rustls`/`ring`; used directly so verifier generation is explicit and
  auditable). `base64` (already a core dependency) handles the URL-safe,
  no-padding encoding.

### Three-Stage, Non-Headless API

Because the flow cannot complete without user interaction, the library exposes
three cooperating pieces rather than a single "authenticate" call:

1. **`PkceChallenge`** — generates/validates the `code_verifier` and derives the
   `code_challenge`. `generate()` produces fresh material from OS entropy;
   `from_verifier()` reconstructs one (validating length + charset) so the
   verifier can survive a redirect round-trip.
2. **`AuthorizeUrlBuilder`** — builds the fully percent-encoded
   `/services/oauth2/authorize` URL (always `response_type=code` + `S256`). The
   caller redirects the user's browser there.
3. **`AuthorizationCode`** — the `Authenticator`. Constructed *after* the
   callback delivers the single-use `code`, it exchanges the code + verifier for
   tokens and thereafter manages the refresh token.

### Per-Authenticator Refresh Token Storage

`AuthorizationCode` stores the refresh token in `Arc<RwLock<Option<SecretString>>>`,
mirroring `UsernamePassword` (see [ADR-025]). This keeps refresh handling an
authenticator-internal concern without touching `AccessToken`, `TokenManager`,
or the `Authenticator` trait.

**Refresh lifecycle:**

1. `authenticate()` — `authorization_code` grant → store refresh token.
2. `refresh()` — use the stored refresh token → update on rotation.
3. Refresh fails — clear the stored token (guarded against clobbering a *newer*
   token stored concurrently), then fall back to `authenticate()`.

**Important caveat vs. username-password:** the authorization `code` is
single-use. Once redeemed, the fallback `authenticate()` in `refresh()` will
fail — the Authorization Code flow **cannot re-authenticate headlessly**. When
that happens the caller must restart the flow with a fresh `PkceChallenge` and
authorize URL. This is inherent to the grant type and is documented on the API.
`set_refresh_token()` / `refresh_token()` accessors let callers persist and
resume the rotating refresh token across process restarts.

### Public vs. Confidential Clients

`client_secret` is modeled as `Option<String>`. Public clients (SPAs, native
apps) pass `None` and rely on PKCE; confidential clients pass `Some(secret)`,
which is appended to the token and refresh requests.

### Token Revocation

`revoke(token)` posts to `/services/oauth2/revoke` (endpoint derived from the
token URL). `revoke_stored_refresh_token()` revokes and clears the stored
refresh token, returning `Ok(())` as a no-op when none is stored.

### Secret Redaction

`AuthorizationCode` and `PkceChallenge` implement manual `Debug` that redacts the
`client_secret`, `code`, `code_verifier`, and PKCE `verifier`. Secrets are held
as `secrecy::SecretString` and never logged, only sent in outbound HTTP bodies.

### Builder Entry Point

`ForceClientBuilder::with_authorization_code(...)` is a feature-gated convenience
over the existing generic `.authenticate(authenticator)` transition, consistent
with how all other authenticators are wired.

## Consequences

### Positive

- Supports Salesforce's recommended interactive-login flow.
- PKCE protects public clients that cannot keep a secret.
- Refresh-token rotation extends sessions without re-prompting the user.
- Feature-gated crypto dependencies keep the default build lean.

### Negative

- The flow is inherently multi-step and cannot be exercised end-to-end without a
  browser, so the example and integration story require manual interaction.
- On refresh-token revocation the flow cannot self-heal; callers must restart the
  interactive flow. This is a property of the grant, not the implementation.
- Adds two dependencies (`sha2`, `getrandom`) behind the feature gate.

### Alternatives Considered

- **`RefreshableAuthenticator<A>` decorator** for shared refresh logic —
  rejected for the same reason as in [ADR-025]: only two flows return refresh
  tokens, so the per-authenticator field is simpler than a decorator.
- **`plain` challenge method** — rejected; Salesforce only accepts `S256` and
  `plain` offers no interception protection.
- **`rand` crate for the verifier** — rejected in favor of `getrandom`, which is
  already in the dependency tree and offers a minimal, OS-backed CSPRNG surface.

## Related

- [ADR-002](002-authentication-strategy.md) — Authentication trait design
- [ADR-025](025-username-password-auth.md) — Refresh-token storage pattern

[RFC 7636]: https://datatracker.ietf.org/doc/html/rfc7636
[ADR-025]: 025-username-password-auth.md
