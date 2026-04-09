//! Capture workers for source systems.

pub(crate) mod postgres;
pub(crate) mod salesforce;

pub use postgres::capture_batch;
pub use salesforce::{capture_stream, load_replay_id};
