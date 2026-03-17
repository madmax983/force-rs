# API Stability and SemVer Policy

## Scope

This policy defines compatibility guarantees for the public `force` crate API.

## SemVer Contract

- `MAJOR`: breaking public API changes.
- `MINOR`: backward-compatible features and expanded support.
- `PATCH`: backward-compatible fixes and documentation/test updates.

Public API includes:

- Public Rust types/functions/traits/modules exported by the crate.
- Behavior contracts documented in public API docs.
- Feature-flag names and their meaning.

## Stability Guarantees

- No breaking change to public API in `PATCH` or `MINOR`.
- New APIs may be added in `MINOR`.
- Bug fixes that alter observable behavior are documented in changelog.

## Feature-Flag Policy

- Existing stable feature flags are treated as public contract.
- Renaming/removing a stable flag is a breaking change.
- Experimental flags may change, but must be clearly documented as experimental.
- Cross-feature interactions must be tested in CI where practical.

## Salesforce API Version Support

- Support multiple Salesforce API versions in a single crate line.
- Do not publish one crate per Salesforce version.
- Version support tiers:
  - current supported versions
  - deprecated versions with removal target
- Upgrades require matrix tests and live-contract validation.

## Deprecation Policy

- Deprecations include migration guidance.
- Deprecation window should span at least one minor release before removal.
- Removals happen only in next major release unless security-critical.

## CI Enforcement

- Lint gate: `cargo clippy --all-targets --all-features -- -D warnings`
- Test gate: `cargo test --all-features -- --test-threads=1`
- Nightly live-contract workflow validates Salesforce integration contracts.

