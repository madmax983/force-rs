//! Smoke tests for the force-sync crate scaffold.

use force_sync::version;

#[test]
fn force_sync_exposes_version() {
    assert_eq!(version(), env!("CARGO_PKG_VERSION"));
}
