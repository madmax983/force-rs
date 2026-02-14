#![allow(missing_docs)]
#![allow(dead_code)]

use force::api::bulk::csv::process_csv_batches;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Record {
    name: String,
}

#[test]
fn test_process_csv_batches_dos() {
    let csv = "name\nAlice\nBob";
    // This should NOT panic or OOM if fixed
    // Currently, it will attempt to allocate usize::MAX capacity, which is guaranteed to fail
    let result = process_csv_batches::<Record, _, _>(csv.as_bytes(), usize::MAX, |_| Ok(()));
    assert!(result.is_ok());
}
