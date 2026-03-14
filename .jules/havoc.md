# 👺 Havoc: TokenManager Token Overwrite

🧨 **The Trigger:** Concurrent token refresh operations where the older (stale) operation yields a token at the exact same microsecond timestamp as a newer token.
📉 **The Stack Trace:** N/A (Data Corruption/Stale Data overwrite)
🧪 **Reproduction:** Run `cargo test havoc`
😈 **Comment:** A strictly greater than `>` sign permitted stale tokens to overwrite equivalently-aged but newer-acquired tokens due to ties in the `issued_at()` epoch validation within `get_token_arc()`. Relaxed to `>=` to preserve existing tokens on ties, protecting freshness. Unconditional forcing behavior remains in `force_refresh()`.
