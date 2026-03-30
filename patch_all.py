import re

# Patch 1: error.rs read_capped_body
with open("crates/force/src/http/error.rs", "r") as f:
    content = f.read()

new_fn = """
/// Reads an HTTP response body up to a maximum size limit to prevent memory exhaustion DoS.
///
/// ⚡ Bolt: Centralizes safely capped HTTP body reading to prevent malicious large chunks
/// from bypassing after-the-fact length checks, while maintaining a single pre-allocated buffer.
pub async fn read_capped_body(response: Response, limit: usize) -> Vec<u8> {
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::with_capacity(4096.min(limit));

    while let Some(chunk_res) = stream.next().await {
        if let Ok(chunk) = chunk_res {
            let remaining = limit.saturating_sub(bytes.len());
            if remaining == 0 {
                break;
            }
            let to_copy = remaining.min(chunk.len());
            bytes.extend_from_slice(&chunk[..to_copy]);

            if bytes.len() >= limit {
                break;
            }
        } else {
            break;
        }
    }
    bytes
}

"""

if "pub async fn read_capped_body" not in content:
    content = content.replace("pub async fn response_to_force_error", new_fn + "pub async fn response_to_force_error")

# Patch 2: error.rs response_to_force_error
old_code_err = """    // Read up to 1MB to prevent memory exhaustion DoS
    let mut stream = response.bytes_stream();
    #[allow(unused_doc_comments)]
    /// ⚡ Bolt: Pre-allocate capacity for the error body to minimize reallocations
    let mut bytes = Vec::with_capacity(4096);
    while let Some(chunk) = stream.next().await {
        if let Ok(chunk_bytes) = chunk {
            bytes.extend_from_slice(&chunk_bytes);
            if bytes.len() > 1024 * 1024 {
                bytes.truncate(1024 * 1024);
                break;
            }
        } else {
            break;
        }
    }
    let body = String::from_utf8_lossy(&bytes).into_owned();"""

new_code_err = """    // Read up to 1MB to prevent memory exhaustion DoS
    let bytes = read_capped_body(response, 1024 * 1024).await;
    let body = String::from_utf8_lossy(&bytes).into_owned();"""

content = content.replace(old_code_err, new_code_err)
with open("crates/force/src/http/error.rs", "w") as f:
    f.write(content)

# Patch 3: auth files
def update_auth_file(filename):
    with open(filename, "r") as f:
        auth_content = f.read()

    old_auth = """            // Read up to 1MB to prevent memory exhaustion DoS
            let mut stream = response.bytes_stream();
            #[allow(unused_doc_comments)]
            /// ⚡ Bolt: Pre-allocate capacity for the error body to minimize reallocations
            let mut bytes = Vec::with_capacity(4096);
            while let Some(chunk) = stream.next().await {
                if let Ok(chunk_bytes) = chunk {
                    bytes.extend_from_slice(&chunk_bytes);
                    if bytes.len() > 1024 * 1024 {
                        bytes.truncate(1024 * 1024);
                        break;
                    }
                } else {
                    break;
                }
            }
            let body = String::from_utf8_lossy(&bytes).into_owned();"""

    new_auth = """            // Read up to 1MB to prevent memory exhaustion DoS
            let bytes = crate::http::error::read_capped_body(response, 1024 * 1024).await;
            let body = String::from_utf8_lossy(&bytes).into_owned();"""

    if old_auth in auth_content:
        auth_content = auth_content.replace(old_auth, new_auth)
        # remove unused import
        auth_content = auth_content.replace("use futures::StreamExt;\n", "")
        with open(filename, "w") as f:
            f.write(auth_content)
        print(f"Patched {filename}")
    else:
        print(f"Could not find old code in {filename}!")

update_auth_file("crates/force/src/auth/jwt_bearer.rs")
update_auth_file("crates/force/src/auth/client_credentials.rs")
