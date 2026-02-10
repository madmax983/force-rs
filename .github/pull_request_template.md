## Summary

- What changed:
- Why:

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-features --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-features -- --test-threads=1`

## Governance Checklist

- [ ] Public API impact assessed against semver policy (`docs/governance/api-stability-policy.md`)
- [ ] Feature-flag behavior/contract updated if applicable
- [ ] Changelog/docs updated for user-visible behavior changes

## Operations Checklist

- [ ] Relevant runbook(s) updated or confirmed:
  - [ ] `docs/runbooks/auth-credential-rotation.md`
  - [ ] `docs/runbooks/rate-limit-incident.md`
  - [ ] `docs/runbooks/retry-tuning.md`
  - [ ] `docs/runbooks/salesforce-api-version-upgrade.md`

## Live Contract Coverage (when applicable)

- [ ] Live tests considered/updated: `crates/force/tests/live_salesforce.rs`
- [ ] Nightly workflow compatibility confirmed: `.github/workflows/live-contract.yml`
- [ ] If intentionally skipped, reason documented below

## Notes

- Risk / rollout notes:
- Follow-ups:

