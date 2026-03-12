# 🗣️ Echo: Getting Started example is broken

🤦 **The Confusion:** Tried to run the "Memory-Efficient Bulk Query" example from the README. The compiler said `next` method was missing on the stream object (`stream.next().await?`).

🕵️ **The Reality:** Turns out I needed to import the `StreamExt` trait (e.g. from `tokio-stream` or `futures`), but the example doesn't mention this anywhere or include the import statement.

💡 **The Fix:** Add `use futures::StreamExt;` (or similar) to the README code block and a comment mentioning the required dependency crate.
