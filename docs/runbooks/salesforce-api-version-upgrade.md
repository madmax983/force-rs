# Runbook: Salesforce API Version Upgrade

## Purpose

Upgrade supported Salesforce API versions with controlled compatibility and release risk.

## Inputs

- Target Salesforce API version (for example `v61.0`)
- Release notes / known API changes
- Existing version compatibility matrix and tests

## Procedure

1. Add/update version support constants and compatibility matrix.
2. Run full local gates:
   - `cargo clippy --all-targets --all-features -- -D warnings`
   - `cargo test --all-features -- --test-threads=1`
3. Run live contract tests in a dev org:
   - REST smoke + negative payload assertions
   - Bulk stream + optional partial-failure test
4. Evaluate breaking behavior:
   - removed fields/endpoints
   - status/error-code changes
   - pagination/locator behavior
5. Update docs and changelog:
   - supported versions
   - migration notes
   - deprecation timelines
6. Release according to semver policy.

## Rollback

- Revert version support update.
- Re-run full tests and publish patch if needed.

## Release Guidance

- Add support for new API versions in minor releases when non-breaking.
- Use major release when crate public API contracts must change.

