# Echo's DX Audit Report 🗣️

I have audited the `force-rs` crate as a new user ("Echo"). Here are my findings:

## 1. Broken Promises (Docs/Code Mismatch) 💔

The `README.md` explicitly lists:
> - **Multiple Auth Flows** - JWT bearer, OAuth 2.0 client credentials, **username-password**
> - **Refresh Token** (session extension) - implied by "Multiple Auth Flows" and often expected.

However, `crates/force/src/auth/mod.rs` only exports:
- `ClientCredentials`
- `JwtBearerFlow` (with `jwt` feature)

There is **no** `UsernamePasswordFlow` or similar exported. This is a major friction point. I tried to use it and it wasn't there.

## 2. Missing Roadmap 🗺️

The `README.md` says:
> See [`ROADMAP.md`](ROADMAP.md) for detailed milestones.

But `ROADMAP.md` does not exist in the repository. This leads to a 404 error if clicked on GitHub.

## 3. Feature Flag Friction 🚧

The Bulk API examples require the `bulk` feature.
```rust
// Requires the "bulk" feature: force = { version = "0.1", features = ["bulk"] }
```
While this is documented in the comment, if a user misses it (like I almost did), the error is:
```
error[E0599]: no method named `bulk` found for struct `ForceClient<A>` in the current scope
```
This is standard Rust behavior but can be confusing for beginners who might think the method should exist but return a "Not Implemented" error or similar.

## 4. Deep Imports 🕳️

The `QueryStream` and `BulkQueryStream` types are returned by public methods but are located deep in the module hierarchy:
- `force::api::rest::query_stream::QueryStream`
- `force::api::bulk::query::BulkQueryStream`

They are not re-exported in `force::api::rest` or `force::api::bulk`, forcing users to either use the deep path or rely on type inference (which works fine for `let mut stream = ...` but not for function signatures).

## Recommendations (Echo's Fixes) 🛠️

1. **Remove False Claims:** Remove "username-password" from README until it is implemented.
2. **Remove Dead Link:** Remove the link to `ROADMAP.md`.
3. **Improve Ergonomics:** Re-export `QueryStream` in `force::api::rest` and `BulkQueryStream` in `force::api::bulk`.

I have implemented these fixes in this PR.
