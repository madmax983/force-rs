# Username-Password Auth Flow Design

## Overview

Feature-gated `username_password` authenticator with refresh token support.
Completes the auth story alongside existing Client Credentials and JWT Bearer.

## Design Decisions

1. **Per-authenticator refresh tokens** — refresh token stored internally in `UsernamePassword`, not a decorator or TokenManager concern
2. **Separate security_token parameter** — library concatenates `password + security_token` to prevent gotcha errors
3. **Feature-gated** — `username_password = []` speed bump for a deprecated flow
4. **Fallback on refresh failure** — if refresh token is revoked/missing, falls back to full re-auth

## Files

```
crates/force/src/auth/
└── username_password.rs   # NEW: UsernamePassword authenticator
```

Modified:
- `Cargo.toml` — add `username_password` feature
- `auth/mod.rs` — feature-gated module + re-export
- `CLAUDE.md` — roadmap + feature list updates

## Public API

```rust
let client = builder()
    .authenticate(UsernamePassword::new_production(
        "client_id", "client_secret",
        "user@example.com", "password", "security_token",
    ))
    .build()
    .await?;
```

## Refresh Token Lifecycle

1. `authenticate()` → POST grant_type=password → store refresh_token from response
2. `refresh()` → POST grant_type=refresh_token → update stored refresh_token
3. Refresh fails → fallback to `authenticate()` (full re-auth)

## Testing

~15-20 wiremock tests: auth success/failure, refresh token storage/rotation/revocation/fallback, constructor variants, credential redaction.
