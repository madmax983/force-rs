//! # force
//!
//! Canonical Salesforce Platform API client for Rust.
//!
//! This crate provides a feature-gated, zero-cost abstraction for all Salesforce Platform APIs.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Allow pedantic/nursery lints that are too strict for this codebase
#![allow(clippy::doc_markdown)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::derive_partial_eq_without_eq)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::format_push_string)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_fields_in_debug)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::needless_continue)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::unwrap_or_default)]
#![allow(clippy::useless_format)]
#![allow(dead_code)] // Phase 2 in progress, some placeholder code exists

pub mod api;
pub mod auth;
pub mod client;
pub mod config;
pub mod error;
pub mod experimental;
pub mod http;
pub(crate) mod session;
#[cfg(test)]
pub(crate) mod test_support;
pub mod types;

/// Force crate placeholder - foundation being built
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(version(), "0.1.0");
    }
}
