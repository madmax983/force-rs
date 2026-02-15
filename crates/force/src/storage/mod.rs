//! Storage and persistence for Salesforce data.
//!
//! This module handles the storage of authentication tokens and other persistent data.
//! It is designed to be decoupled from the core logic to allow for different storage backends.

pub mod token;
pub use token::{AccessToken, TokenManager, TokenResponse};
