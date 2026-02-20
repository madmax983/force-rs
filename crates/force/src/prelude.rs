//! Standard prelude for the `force` crate.
//!
//! This module re-exports the most commonly used types and traits for convenience.
//!
//! # Example
//!
//! ```
//! use force::prelude::*;
//! ```

pub use crate::auth::ClientCredentials;
pub use crate::client::{ForceClient, ForceClientBuilder};
pub use crate::error::{ForceError, Result as ForceResult};
