# 🗣️ Echo: Getting Started example is broken

**Role:** Echo 🗣️

## 🤦 The Confusion
I copy-pasted the "Query Plan API" example from the `README.md` into my project. I hit `cargo run` and the compiler yelled at me! I thought the library was broken or the docs were outdated.
On top of that, while trying to read the docs for `ForceClient` (`crates/force/src/client/mod.rs` and `crates/force/src/client/builder.rs`), I saw stuff about "Phantom types", "compile-time authentication safety", "zero-cost abstraction", "marker type", and "generic over the authenticator". What does that even mean? I just want to query Salesforce!

## 🕵️ The Reality
Turns out, I needed to enable the `nova` feature in my `Cargo.toml`. The `README.md` does mention it, but the comment `// Requires the "nova" feature: force = { version = "0.1", features = ["nova"] }` is buried inside the `main()` function instead of right at the top where I'd see it before copying the imports.
As for the jargon, the library is safe and fast, but developers shouldn't have to read compiler theory to figure out how to use an API client.

## 💡 The Fix
- Move the `// Requires the "nova" feature` comment to the very top of the code snippet in `README.md`, before the imports. Don't hide important feature requirements inside the function body!
- Remove the jargon ("zero-cost abstraction", "phantom types", "compile-time safety", "marker type", "generic over the authenticator") from `crates/force/src/client/mod.rs` and `crates/force/src/client/builder.rs` docs and keep the language simple.
