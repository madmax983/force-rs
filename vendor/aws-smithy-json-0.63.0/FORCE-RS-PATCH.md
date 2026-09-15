# Local patch: force-rs

This is the unmodified upstream `aws-smithy-json` 0.63.0 source (Apache-2.0,
<https://github.com/smithy-lang/smithy-rs>) with three one-line fixes so it
compiles against `aws-smithy-types` 1.7.0's `Document::Object(DocumentObject)`
shape, which 0.63.0 predates (it still builds `Document::Object` from a raw
`HashMap<String, Document>`). No published `aws-config`/`aws-sdk-*` release
yet raises its `aws-smithy-json` requirement above `^0.63.0` to pick up the
fixed `0.64.0`, and `aws-smithy-types` is independently forced to `1.7.0` by
`aws-smithy-runtime`, so the workspace can't route around this by pinning
versions alone. See `[patch.crates-io]` in the workspace root `Cargo.toml`.

Changes from upstream 0.63.0, each exactly what `rustc` itself suggested:

- `src/codec/deserializer.rs`: `Document::Object(map)` -> `Document::Object(map.into())`
- `src/deserialize/token.rs`: `Document::Object(object)` -> `Document::Object(object.into())`
- `src/serialize.rs`: added a wildcard match arm (`_ => self.null()`) for
  `Document` being `#[non_exhaustive]`

Both `.into()` calls use `aws-smithy-types`'s own
`impl From<HashMap<String, Document>> for DocumentObject`, so they're a
lossless, upstream-sanctioned conversion, not a workaround.

**Remove this vendor directory and the `[patch.crates-io]` entry** once
crates.io has an `aws-config`/`aws-sdk-*` release that requires
`aws-smithy-json >= 0.64`.
