//! API surface handlers for Marketing Cloud Engagement.
//!
//! Each submodule provides a lightweight handler, obtained from the client, that
//! borrows the client and issues typed requests against one API family.

pub(crate) mod assets;
pub(crate) mod contacts;
pub(crate) mod data_extensions;
pub(crate) mod journeys;
pub(crate) mod transactional;
