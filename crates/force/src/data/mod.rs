//! Data generation and seeding utilities.

pub(crate) mod data_archiver;
pub(crate) mod data_faker;
pub(crate) mod data_seeder;

pub use data_archiver::archive_to_jsonl;
pub use data_faker::generate_mock_record;
pub use data_seeder::seed_data;
