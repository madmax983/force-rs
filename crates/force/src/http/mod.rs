//! HTTP client layer with middleware for retry, rate limiting, and auth refresh.
//!
//! This module provides the HTTP execution layer that handles:
//! - Automatic token injection and Bearer authentication
//! - 401 response detection and automatic token refresh + retry
//! - 429 rate limit handling with Retry-After header respect
//! - Exponential backoff for retryable errors (503, network failures)
//! - Sforce-Limit-Info header tracking for API limits
//! - Detailed error parsing from Salesforce API responses

pub mod error;
pub mod executor;
pub mod retry;

pub(crate) use error::response_to_force_error;
pub use executor::HttpExecutor;
pub use retry::{RequestRetryClass, RetryPolicy};

#[cfg(test)]
mod tests;
