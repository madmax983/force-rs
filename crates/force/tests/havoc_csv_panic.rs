#![allow(clippy::expect_used)]
#![allow(missing_docs)]
#![cfg(feature = "bulk")]

use force::api::bulk::process_csv_batches;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Record {
    id: String,
}

#[test]
fn test_process_csv_batches_panic() {
    let csv_data = "id\n1";

    // This previously panicked due to Vec::with_capacity(usize::MAX).
    // It should now run safely (albeit with a potentially large batch if we had enough data).
    let _ = process_csv_batches(
        csv_data.as_bytes(),
        usize::MAX,
        |_batch: Vec<Record>| Ok(()),
    );
}
