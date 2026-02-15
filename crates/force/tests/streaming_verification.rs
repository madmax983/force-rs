#![cfg(feature = "bulk")]
#![allow(missing_docs)]

use futures::StreamExt;
use tokio_util::compat::TokioAsyncReadCompatExt;

#[derive(serde::Deserialize, Debug)]
struct Record {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
}

#[tokio::test]
#[allow(clippy::expect_used, clippy::similar_names)]
async fn test_streaming_csv_pipeline() {
    // Simulate a CSV stream with two records
    let data = "Id,Name\n001,Acme\n002,Globex\n";
    // Construct a stream that mimics reqwest stream (bytes)
    // We yield it as one chunk to verify basic parsing
    let stream = futures::stream::iter(vec![Ok::<bytes::Bytes, std::io::Error>(
        bytes::Bytes::from(data),
    )]);

    // Pipeline: stream -> StreamReader -> compat -> csv-async
    let reader = tokio_util::io::StreamReader::new(stream).compat();

    let csv_reader = csv_async::AsyncReaderBuilder::new().create_deserializer(reader);

    let mut records = csv_reader.into_deserialize::<Record>();

    // 1. Read Record 1
    let record1 = records
        .next()
        .await
        .expect("Stream should have record 1")
        .expect("Record 1 should be valid");
    assert_eq!(record1.id, "001");
    assert_eq!(record1.name, "Acme");

    // 2. Read Record 2
    let record2 = records
        .next()
        .await
        .expect("Stream should have record 2")
        .expect("Record 2 should be valid");
    assert_eq!(record2.id, "002");
    assert_eq!(record2.name, "Globex");

    // 3. End of stream
    assert!(records.next().await.is_none());
}
