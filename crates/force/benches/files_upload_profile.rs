//! One-shot profiling harness for `FilesHandler::upload`
//! (Salesforce Files / `ContentVersion` multipart upload).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `client.files().upload()` entry point, meant to be executed once
//! under `valgrind --tool=callgrind` (instruction counts) and
//! `valgrind --tool=dhat` (allocation bytes/counts) so those tools attribute
//! cost to real call stacks instead of a criterion harness loop.
//!
//! Workload: upload `FILE_COUNT` files of `FILE_SIZE` bytes each (a realistic
//! size for a generated PDF/image/spreadsheet attachment) through the public
//! `client.files().upload()` entry point against an in-process mock Salesforce
//! endpoint that accepts every multipart POST with a 201. File contents are a
//! deterministic (non-RNG) byte pattern so repeated runs are byte-for-byte
//! identical, which is required for callgrind/dhat comparisons to be
//! meaningful.
//!
//! `upload()`'s request-building closure (`make_request`, called at least
//! once per attempt so the executor can rebuild a non-clonable multipart body
//! on retry) clones the full file buffer on every call. This harness has no
//! retries -- it isolates the cost of that one unconditional clone per
//! upload, which is exactly what every successful, non-retried upload pays
//! today.
//!
//! Run directly (`cargo run` doesn't support `--bench`, so use `cargo bench`):
//! ```bash
//! cargo bench -p force --features files --bench files_upload_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --features files --bench files_upload_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'files_upload_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use force::auth::{AccessToken, Authenticator, TokenResponse};
use force::client::builder;
use force::error::Result as ForceResult;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Size of each uploaded file, in bytes -- a realistic size for a generated
/// PDF, image, or spreadsheet `ContentVersion` attachment.
const FILE_SIZE: usize = 4 * 1024 * 1024;

/// Number of files uploaded in one profiling run.
const FILE_COUNT: usize = 10;

#[derive(Debug, Clone)]
struct StaticAuthenticator {
    token: String,
    instance_url: String,
}

#[async_trait::async_trait]
impl Authenticator for StaticAuthenticator {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new(self.token.clone().into()),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "profile-sig".to_string(),
            expires_in: Some(7200),
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}

/// Builds a deterministic (no RNG), non-trivial byte pattern for one file so
/// repeated runs are byte-for-byte identical.
fn file_bytes(seed: u32) -> Vec<u8> {
    let mut buf = vec![0u8; FILE_SIZE];
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    for byte in &mut buf {
        // xorshift32 -- cheap, deterministic, avoids an all-zero/all-same buffer.
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *byte = (state & 0xFF) as u8;
    }
    buf
}

async fn run() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/services/data/v67.0/sobjects/ContentVersion"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "068000000000001AAA",
            "success": true,
            "errors": []
        })))
        .mount(&mock_server)
        .await;

    let auth = StaticAuthenticator {
        token: "profile_token".to_string(),
        instance_url: mock_server.uri(),
    };
    let client = builder()
        .authenticate(auth)
        .build()
        .await
        .expect("client build");

    for i in 0..u32::try_from(FILE_COUNT).expect("FILE_COUNT fits in u32") {
        let bytes = file_bytes(i);
        let title = format!("Profiling Attachment {i}");
        let path_on_client = format!("attachment-{i}.bin");

        client
            .files()
            .upload(&title, &path_on_client, bytes)
            .await
            .expect("upload should succeed");
    }
}

fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(run());
}
