#![allow(missing_docs)]
#![allow(dead_code)]

#[cfg(feature = "bulk")]
use force::api::bulk::csv::process_csv_batches;
#[cfg(feature = "bulk")]
use serde::Deserialize;

#[cfg(feature = "bulk")]
#[derive(Deserialize, Debug)]
struct Record {
    name: String,
}

#[cfg(feature = "bulk")]
#[test]
fn test_process_csv_batches_dos() {
    let csv = "name\nAlice\nBob";
    // This should NOT panic or OOM if fixed
    // Currently, it will attempt to allocate usize::MAX capacity, which is guaranteed to fail
    let result = process_csv_batches::<Record, _, _>(csv.as_bytes(), usize::MAX, |_| Ok(()));
    assert!(result.is_ok());
}
