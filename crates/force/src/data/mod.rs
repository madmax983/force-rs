//! Data generation and seeding utilities.

pub(crate) mod data_archiver;
pub(crate) mod data_faker;
pub(crate) mod data_masker;
pub(crate) mod data_seeder;
pub(crate) mod data_validator;

pub use data_archiver::DataArchiver;
pub use data_faker::{generate_mock_query, generate_mock_record};
pub use data_masker::DataMasker;
pub use data_seeder::DataSeeder;
pub use data_validator::{DataValidator, ValidationError};
#[cfg(feature = "data_utility")]
pub(crate) mod data_profiler;
#[cfg(feature = "data_utility")]
pub use data_profiler::{DataProfileReport, DataProfiler, FieldProfile};

#[cfg(all(feature = "data_utility", feature = "composite_graph"))]
pub(crate) mod relational_seeder;
#[cfg(all(feature = "data_utility", feature = "composite_graph"))]
pub use relational_seeder::RelationalSeeder;
