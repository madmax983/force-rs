//! OAuth 2.0 Authorization Code + PKCE Example
//!
//! Demonstrates the interactive (browser-based) Authorization Code flow with
//! Proof Key for Code Exchange (PKCE), Salesforce's recommended flow for web
//! and native apps.
//!
//! Because this flow requires a human to log in through a browser, the example
//! runs in three stages:
//!
//! 1. Generate a PKCE verifier/challenge and print the authorization URL.
//! 2. You open that URL, log in, and Salesforce redirects your browser to the
//!    configured `redirect_uri` with a `?code=...` query parameter. Paste that
//!    code back in (via the `SF_AUTH_CODE` env var or stdin).
//! 3. The example exchanges the code for tokens and makes an authenticated call.
//!
//! # Setup
//!
//! Register a Connected App with the Authorization Code flow enabled and a
//! callback URL, then set:
//! - `SF_CLIENT_ID`     - OAuth client ID (Connected App Consumer Key)
//! - `SF_REDIRECT_URI`  - Callback URL registered on the Connected App
//! - `SF_CLIENT_SECRET` - (optional) client secret for confidential clients
//! - `SF_LOGIN_URL`     - (optional) login host, defaults to
//!   `https://login.salesforce.com`
//! - `SF_AUTH_CODE`     - (optional) the authorization code; if unset the
//!   example reads it from stdin
//!
//! # Run
//!
//! ```bash
//! SF_CLIENT_ID=xxx SF_REDIRECT_URI=https://your.app/callback \
//!     cargo run --example auth_code_pkce --features auth_code
//! ```

#[cfg(feature = "auth_code")]
mod example {
    use anyhow::{Context, Result};
    use force::api::RestOperation;
    use force::auth::{AuthorizationCode, AuthorizeUrlBuilder, PkceChallenge};
    use force::client::ForceClientBuilder;
    use force::types::QueryResult;

    fn required_env(name: &str) -> Result<String> {
        std::env::var(name).with_context(|| format!("{name} environment variable not set"))
    }

    pub async fn run() -> Result<()> {
        let client_id = required_env("SF_CLIENT_ID")?;
        let redirect_uri = required_env("SF_REDIRECT_URI")?;
        let client_secret = std::env::var("SF_CLIENT_SECRET").ok();
        let login_url = std::env::var("SF_LOGIN_URL")
            .unwrap_or_else(|_| "https://login.salesforce.com".to_string());
        let token_url = format!("{}/services/oauth2/token", login_url.trim_end_matches('/'));

        // ── Stage 1: generate PKCE material + authorize URL ─────────────
        let pkce = PkceChallenge::generate();
        let authorize_url = AuthorizeUrlBuilder::new(&login_url, &client_id, &redirect_uri, &pkce)
            .scope("api")
            .scope("refresh_token")
            .state("example-state-token")
            .build();

        println!("=== Stage 1: Authorize ===");
        println!("Open this URL in your browser and log in:\n");
        println!("  {authorize_url}\n");
        println!(
            "After approving, Salesforce redirects to your redirect_uri with a\n\
             `?code=...` parameter. Copy that code value.\n"
        );

        // ── Stage 2: obtain the authorization code ──────────────────────
        let code = match std::env::var("SF_AUTH_CODE") {
            Ok(code) if !code.trim().is_empty() => code.trim().to_string(),
            _ => {
                println!("Paste the authorization code and press Enter:");
                let mut line = String::new();
                std::io::stdin()
                    .read_line(&mut line)
                    .context("failed to read authorization code from stdin")?;
                line.trim().to_string()
            }
        };

        if code.is_empty() {
            anyhow::bail!("no authorization code provided");
        }

        // ── Stage 3: exchange the code for tokens + make a call ─────────
        println!("\n=== Stage 3: Token Exchange ===");
        let auth = AuthorizationCode::new(
            client_id,
            client_secret,
            redirect_uri,
            code,
            pkce.verifier(),
            token_url,
        );

        let client = ForceClientBuilder::new().authenticate(auth).build().await?;
        println!("Token exchange successful.\n");

        println!("=== Authenticated Query ===");
        let result: QueryResult<serde_json::Value> = client
            .rest()
            .query("SELECT Id, Name FROM Account ORDER BY CreatedDate DESC LIMIT 5")
            .await?;
        println!("Found {} account(s):", result.total_size);
        for record in &result.records {
            println!("  - {}", record["Name"].as_str().unwrap_or("(no name)"));
        }

        Ok(())
    }
}

#[cfg(feature = "auth_code")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    example::run().await
}

#[cfg(not(feature = "auth_code"))]
fn main() {
    eprintln!("This example requires the 'auth_code' feature.");
    eprintln!("Run with: cargo run --example auth_code_pkce --features auth_code");
}
