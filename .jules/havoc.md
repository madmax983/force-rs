# 👺 Havoc: TokenManager Token Overwrite

🧨 **The Trigger:** Consecutive `force_refresh` and `get_token` with tokens generated at exactly the same microsecond.
📉 **The Stack Trace:** N/A (Data Corruption/Stale Data overwrite)
🧪 **Reproduction:** Run `cargo test havoc`
😈 **Comment:** A strictly greater than `>` sign permitted stale tokens to overwrite equivalently-aged but newer-acquired tokens due to ties in the `issued_at()` epoch validation. Relaxed to `>=`.
