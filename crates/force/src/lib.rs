//! # force
//!
//! Canonical Salesforce Platform API client for Rust.
//!
//! This crate provides a feature-gated, zero-cost abstraction for all Salesforce Platform APIs.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod auth;
pub mod client;
pub mod config;
pub mod error;
pub mod http;
pub mod types;

/// Force crate placeholder - foundation being built
pub fn version() -> &'static str {
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
