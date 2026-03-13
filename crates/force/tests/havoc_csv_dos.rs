#![allow(missing_docs)]

use serde::Deserialize;
use std::io::{Read, Result as IoResult};

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct TestRecord {
    id: String,
    name: String,
}

// An infinite reader to test memory exhaustion bounds.
struct InfiniteCsvReader {
    count: usize,
}

impl Read for InfiniteCsvReader {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.count == 0 {
            let header = b"id,name\n";
            let len = std::cmp::min(buf.len(), header.len());
            buf[..len].copy_from_slice(&header[..len]);
            self.count += len;
            return Ok(len);
        }

        let row = b"1234567890,Some long name that repeats\n";
        let mut written = 0;
        while written < buf.len() {
            let remaining = buf.len() - written;
            let take = std::cmp::min(remaining, row.len());
            buf[written..written + take].copy_from_slice(&row[..take]);
            written += take;
        }

        self.count += written;
        Ok(written)
    }
}

#[cfg(feature = "bulk")]
#[test]
fn havoc_csv_memory_limit() {
    let reader = InfiniteCsvReader { count: 0 };

    let res: force::error::Result<Vec<TestRecord>> =
        force::api::bulk::csv::deserialize_from_csv(reader);

    assert!(
        res.is_err(),
        "Expected memory limit error, but succeeded (or OOM'd)"
    );

    // Check if error is IO ErrorKind::InvalidData
    let Err(err) = res else {
        panic!("Expected an error result");
    };

    let msg = format!("{err:?}");
    assert!(
        msg.contains("Payload exceeded maximum size limit") || msg.contains("InvalidData"),
        "Expected InvalidData error, got: {msg}"
    );
}
