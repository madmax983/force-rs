# 👺 Havoc: TokenManager Token Overwrite

🧨 **The Trigger:** Concurrent token refresh operations where the older (stale) operation yields a token at the exact same microsecond timestamp as a newer token.
📉 **The Stack Trace:** N/A (Data Corruption/Stale Data overwrite)
🧪 **Reproduction:** Run `cargo test havoc`
😈 **Comment:** A strictly greater than `>` sign permitted stale tokens to overwrite equivalently-aged but newer-acquired tokens due to ties in the `issued_at()` epoch validation within `get_token_arc()`. Relaxed to `>=` to preserve existing tokens on ties, protecting freshness. Unconditional forcing behavior remains in `force_refresh()`.
**2024-11-20 - [UsernamePassword Refresh Token Race Condition]**
**Threat:** [The UsernamePassword authenticator was vulnerable to a race condition where a failing `refresh()` call would blindly clear the `refresh_token` state without checking if another concurrent task had authenticated and injected a new, valid token. This could cause the valid token to be permanently lost and lead to unnecessary full re-authentications.]
**Defense:** [Added a strict equality check inside `refresh()` before assigning `None` to the `refresh_token`. The state is only cleared if it hasn't changed since the failing network request started.]
