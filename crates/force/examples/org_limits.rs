//! Org Limits Example
//!
//! This example demonstrates how to check organization limits and API usage.
//!
//! # Setup
//!
//! Set the following environment variables:
//! - `SF_CLIENT_ID` - OAuth client ID
//! - `SF_CLIENT_SECRET` - OAuth client secret
//!
//! # Run
//!
//! ```bash
//! cargo run --example org_limits --features rest
//! ```

#[cfg(feature = "rest")]
mod example {
    use anyhow::Context;
    use force::api::rest::limits::OrgLimits;
    use force::auth::ClientCredentials;
    use force::client::builder;

    fn required_env(name: &str) -> anyhow::Result<String> {
        std::env::var(name).with_context(|| format!("{name} environment variable not set"))
    }

    #[tokio::main]
    pub async fn main() -> anyhow::Result<()> {
        // Initialize tracing
        tracing_subscriber::fmt::init();

        // Get credentials from environment
        let client_id = required_env("SF_CLIENT_ID")?;
        let client_secret = required_env("SF_CLIENT_SECRET")?;

        println!("═══ Authenticating ═══");
        let auth = ClientCredentials::new(
            client_id,
            client_secret,
            "https://login.salesforce.com/services/oauth2/token",
        );
        let client = builder().authenticate(auth).build().await?;
        println!("✓ Authentication successful\n");

        println!("═══ Checking Org Limits ═══");
        let limits = client.rest().limits().await?;

        // 1. API Requests Limit (Most important for integrations)
        println!("\n[Daily API Requests]");
        print_limit_bar(&limits.daily_api_requests);

        // 2. Data Storage
        if let Some(storage) = limits.get("DataStorageMB") {
            println!("\n[Data Storage (MB)]");
            print_limit_bar(storage);
        }

        // 3. File Storage
        if let Some(files) = limits.get("FileStorageMB") {
            println!("\n[File Storage (MB)]");
            print_limit_bar(files);
        }

        // 4. Bulk API Limits (if available)
        println!("\n[Bulk API]");
        if let Some(bulk) = limits.get("DailyBulkApiRequests") {
            print_limit_bar(bulk);
        }

        // 5. Streaming API Limits
        println!("\n[Streaming API]");
        if let Some(streaming) = limits.get("DailyDurableStreamingApiEvents") {
            println!("Durable Events:");
            print_limit_bar(streaming);
        }

        // Warning check
        println!("\n═══ Health Check ═══");
        let warnings = check_warnings(&limits);
        if warnings.is_empty() {
            println!("✓ All limits are within healthy ranges.");
        } else {
            println!("⚠ Warnings found:");
            for warning in warnings {
                println!("  - {warning}");
            }
        }

        Ok(())
    }

    fn print_limit_bar(info: &force::api::rest::limits::LimitInfo) {
        let percent = info.percentage_used();
        let bar_width: usize = 30;
        let filled = (percent / 100.0 * bar_width as f64).round() as usize;
        let empty = bar_width.saturating_sub(filled);

        let bar = format!(
            "[{}{}] {:.1}%",
            "█".repeat(filled),
            "░".repeat(empty),
            percent
        );

        println!("{:<30} {} / {}", bar, info.remaining, info.max);
    }

    fn check_warnings(limits: &OrgLimits) -> Vec<String> {
        let mut warnings = Vec::new();
        let threshold = 80.0; // Warn if usage > 80%

        // Check explicit limits
        if limits.daily_api_requests.is_above_threshold(threshold) {
            warnings.push(format!(
                "Daily API requests high usage: {:.1}%",
                limits.daily_api_requests.percentage_used()
            ));
        }

        // Check all other limits
        for (name, info) in &limits.additional_limits {
            if info.is_above_threshold(threshold) {
                warnings.push(format!(
                    "{name} high usage: {:.1}%",
                    info.percentage_used()
                ));
            }
        }

        warnings
    }

    trait LimitMapExt {
        fn get(&self, key: &str) -> Option<&force::api::rest::limits::LimitInfo>;
    }

    impl LimitMapExt for OrgLimits {
        fn get(&self, key: &str) -> Option<&force::api::rest::limits::LimitInfo> {
            self.additional_limits.get(key)
        }
    }
}

#[cfg(feature = "rest")]
fn main() -> anyhow::Result<()> {
    example::main()
}

#[cfg(not(feature = "rest"))]
fn main() {
    println!("This example requires the 'rest' feature.");
}
