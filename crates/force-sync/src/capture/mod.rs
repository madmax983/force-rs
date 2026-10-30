//! Capture workers for source systems.

pub(crate) mod postgres;
pub(crate) mod salesforce;

pub use postgres::capture_batch;
pub use salesforce::{
    CdcEventStream, CdcSource, CdcStart, CdcStream, CdcWindow, CdcWindowEnd, CdcWindowReport,
    FIRST_POSITION_WAIT, capture_cdc_window, capture_stream, load_replay_id,
};
