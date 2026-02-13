#![cfg(feature = "bulk")]
#![allow(missing_docs)]

use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use async_trait::async_trait;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use serde::Deserialize;

// Mock authenticator that points to our local server
#[derive(Debug, Clone)]
struct LocalAuthenticator {
    port: u16,
}

#[async_trait]
impl Authenticator for LocalAuthenticator {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: "test_token".to_string(),
            instance_url: format!("http://127.0.0.1:{}", self.port),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "test_sig".to_string(),
            expires_in: Some(7200),
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> force::error::Result<AccessToken> {
        self.authenticate().await
    }
}

#[derive(Debug, Deserialize, PartialEq)]
struct TestRecord {
    id: String,
    name: String,
}

#[tokio::test]
async fn test_bulk_query_streaming_behavior() {
    // strict chaos mode: fail if not streaming
    // verified: this test passes with the streaming implementation

    // 1. Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("Failed to bind");
    let port = listener.local_addr().unwrap().port();

    // 2. Spawn the server task
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("Failed to accept");

        // Read request (simplified)
        let mut buf = [0; 1024];
        let _ = socket.read(&mut buf).await;

        // Write HTTP response headers
        let response_headers = "HTTP/1.1 200 OK\r\n\
                                Content-Type: text/csv\r\n\
                                Transfer-Encoding: chunked\r\n\
                                \r\n";
        socket.write_all(response_headers.as_bytes()).await.expect("Failed to write headers");

        // Chunk 1: Header + First Record
        let chunk1 = "Id,Name\n001xx0000000001AAA,FastRecord\n";
        let chunk1_len = format!("{:x}\r\n", chunk1.len());
        socket.write_all(chunk1_len.as_bytes()).await.expect("Failed to write chunk1 len");
        socket.write_all(chunk1.as_bytes()).await.expect("Failed to write chunk1");
        socket.write_all(b"\r\n").await.expect("Failed to write chunk1 end");
        socket.flush().await.expect("Failed to flush chunk1");

        // Delay to simulate slow stream
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Chunk 2: Second Record
        let chunk2 = "001xx0000000002AAA,SlowRecord\n";
        let chunk2_len = format!("{:x}\r\n", chunk2.len());
        socket.write_all(chunk2_len.as_bytes()).await.expect("Failed to write chunk2 len");
        socket.write_all(chunk2.as_bytes()).await.expect("Failed to write chunk2");
        socket.write_all(b"\r\n").await.expect("Failed to write chunk2 end");

        // End of stream
        socket.write_all(b"0\r\n\r\n").await.expect("Failed to write end of stream");
    });

    // 3. Setup client
    let auth = LocalAuthenticator { port };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("Failed to build client");

    // 4. Create stream
    let mut stream = client.bulk()
        .query_results::<TestRecord>("test_job_id")
        .await
        .expect("Failed to create stream");

    // 5. Test streaming behavior
    // The server sends the first record immediately, then sleeps 3s.
    // If implementation buffers, next() will take > 3s.
    // If implementation streams, next() will take < 1s.

    let next_future = stream.next();

    // We expect the first record within 1s
    let result = tokio::time::timeout(Duration::from_secs(1), next_future).await;

    match result {
        Ok(Ok(Some(record))) => {
            assert_eq!(record.id, "001xx0000000001AAA");
            assert_eq!(record.name, "FastRecord");
            println!("SUCCESS: Stream returned first record immediately.");
        },
        Ok(Ok(None)) => panic!("Stream ended too early"),
        Ok(Err(e)) => panic!("Stream error: {}", e),
        Err(_) => panic!("FAILURE: Stream blocked waiting for full response (Buffering detected!)"),
    }
}
