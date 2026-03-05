# 🗣️ Echo: Getting Started example is broken

**Role:** Echo 🗣️
**Date:** 2026-02-22

## 🤦 The Confusion
I copy-pasted the Query Plan API example from the `README.md` into my project. I hit `cargo run` and the compiler yelled at me:
```text
error[E0599]: no method named explain found for struct RestHandler<ClientCredentials> in the current scope
```
I thought the library was broken or the docs were outdated!

On top of that, while trying to read the docs for `ForceClient` (`crates/force/src/client/mod.rs`), I saw stuff about "phantom types", "compile-time authentication safety", and "zero-cost abstraction". What does that even mean? I just want to query Salesforce!

## 🕵️ The Reality
Turns out, I needed to enable the `nova` feature in my `Cargo.toml`. The `README.md` *does* mention it, but the comment `// Requires the "nova" feature: force = { version = "0.1", features = ["nova"] }` is buried inside the `main()` function, on line 14, instead of right at the top where I'd see it before copying the imports.

As for the jargon, the library is safe and fast, but developers shouldn't have to read compiler theory to figure out how to use an API client.

## 💡 The Fix
- Move the `// Requires the "nova" feature` comment to the very top of the code snippet in `README.md`, just like the Bulk API examples do. Don't hide important feature requirements inside the function body!
- Remove the "zero-cost abstraction", "phantom types", and "compile-time safety" jargon from `crates/force/src/client/mod.rs` docs and keep the language simple.