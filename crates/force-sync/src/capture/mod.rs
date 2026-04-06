//! Capture workers for source systems.

pub mod postgres;
pub mod salesforce;

pub use postgres::capture_batch;
pub use salesforce::{capture_stream, load_replay_id};
