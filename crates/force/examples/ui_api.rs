//! UI API Example
//!
//! Demonstrates using the Salesforce UI API for layout-aware record operations,
//! object metadata, list views, lookups, and favorites.
//!
//! Unlike the REST API (which returns raw SObject data), the UI API returns
//! presentation-ready structures — field display values, layout sections,
//! list view columns — exactly what a front-end needs to render a record page.
//!
//! # Setup
//!
//! Set the following environment variables:
//! - `SF_CLIENT_ID`      - OAuth client ID (Connected App Consumer Key)
//! - `SF_CLIENT_SECRET`  - OAuth client secret (Connected App Consumer Secret)
//! - `SF_RECORD_ID`      - A valid Account ID (15 or 18 chars) — optional
//!
//! # Run
//!
//! ```bash
//! SF_CLIENT_ID=xxx SF_CLIENT_SECRET=yyy SF_RECORD_ID=001... \
//!   cargo run --example ui_api --features ui
//! ```

#[cfg(feature = "ui")]
mod example {
    use anyhow::{Context, Result};
    use force::api::ui::{FavoriteInput, LayoutType, Mode};
    use force::auth::ClientCredentials;
    use force::client::ForceClientBuilder;
    use std::env;

    pub async fn run() -> Result<()> {
        let client_id = env::var("SF_CLIENT_ID").context("SF_CLIENT_ID not set")?;
        let client_secret = env::var("SF_CLIENT_SECRET").context("SF_CLIENT_SECRET not set")?;
        let record_id = env::var("SF_RECORD_ID")
            .unwrap_or_else(|_| "001000000000001AAA".to_string());

        let auth = ClientCredentials::new_production(client_id, client_secret);
        let client = ForceClientBuilder::new()
            .authenticate(auth)
            .build()
            .await
            .context("Failed to build client")?;

        let ui = client.ui();

        // ── Object Info ───────────────────────────────────────────────────────
        println!("=== Object Info: Account ===");
        let info = ui.object_info("Account").await?;
        println!("  Label:        {}", info.label);
        println!("  Label Plural: {}", info.label_plural);
        println!("  Key Prefix:   {:?}", info.key_prefix);
        println!("  Fields:       {} field(s)", info.fields.len());
        if let Some(name_field) = info.fields.get("Name") {
            println!(
                "  Name field — required={} updateable={}",
                name_field.required, name_field.updateable
            );
        }
        println!();

        // ── Record UI (layout-aware) ──────────────────────────────────────────
        println!("=== Record UI: {record_id} ===");
        match ui
            .record_ui(
                &[record_id.as_str()],
                Some(&[LayoutType::Full]),
                Some(&[Mode::View]),
            )
            .await
        {
            Ok(record_ui) => {
                println!("  Records in response: {}", record_ui.records.len());
                if let Some(record) = record_ui.records.get(&record_id) {
                    println!("  API name: {}", record.api_name);
                    println!("  Fields:   {} field(s)", record.fields.len());
                    if let Some(name_field) = record.fields.get("Name") {
                        println!("  Name:     {:?}", name_field.display_value);
                    }
                }
            }
            Err(e) => println!("  (skipped — record not found or inaccessible: {e})"),
        }
        println!();

        // ── Layout ────────────────────────────────────────────────────────────
        println!("=== Layout: Account (Full, View) ===");
        match ui
            .layout("Account", Some(&LayoutType::Full), Some(&Mode::View))
            .await
        {
            Ok(layout) => {
                println!("  Layout ID:   {}", layout.id);
                println!("  Layout type: {}", layout.layout_type);
                println!("  Mode:        {}", layout.mode);
                println!("  Sections:    {}", layout.sections.len());
                for section in &layout.sections {
                    let heading = section.heading.as_deref().unwrap_or("(no heading)");
                    println!(
                        "    Section: {} ({} columns, {} rows)",
                        heading, section.columns, section.rows
                    );
                }
            }
            Err(e) => println!("  (skipped: {e})"),
        }
        println!();

        // ── List Views ────────────────────────────────────────────────────────
        println!("=== List Views: Account ===");
        match ui.list_views("Account").await {
            Ok(collection) => {
                println!("  Total views: {}", collection.count);
                for lv in collection.list_views.iter().take(3) {
                    println!("  - {} ({})", lv.label, lv.api_name);
                }
            }
            Err(e) => println!("  (skipped: {e})"),
        }
        println!();

        // ── Lookup ────────────────────────────────────────────────────────────
        println!("=== Lookup: Contact.AccountId for 'Acme' ===");
        match ui.lookup("Contact", "AccountId", "Acme").await {
            Ok(results) => {
                println!("  Found {} result(s)", results.count);
                for entry in results.lookup_results.iter().take(5) {
                    println!("  - {} ({})", entry.label, entry.id);
                }
            }
            Err(e) => println!("  (skipped: {e})"),
        }
        println!();

        // ── Favorites ─────────────────────────────────────────────────────────
        println!("=== Favorites ===");
        let favs = ui.get_favorites().await?;
        println!("  Current favorites: {}", favs.favorites.len());
        for fav in favs.favorites.iter().take(3) {
            println!("  - {} (type: {:?})", fav.label, fav.target_type);
        }

        println!("\n  Creating test favorite...");
        let input = FavoriteInput {
            label: "UI API Example Favorite".to_string(),
            name: None,
            target: record_id.clone(),
            target_type: "Record".to_string(),
        };
        match ui.create_favorite(&input).await {
            Ok(created) => {
                println!("  Created: id={} label={}", created.id, created.label);

                let update_input = FavoriteInput {
                    label: "Renamed by UI API Example".to_string(),
                    ..input
                };
                match ui.update_favorite(&created.id, &update_input).await {
                    Ok(updated) => println!("  Updated label: {}", updated.label),
                    Err(e) => println!("  Update failed: {e}"),
                }

                match ui.delete_favorite(&created.id).await {
                    Ok(()) => println!("  Deleted favorite: {}", created.id),
                    Err(e) => println!("  Delete failed: {e}"),
                }
            }
            Err(e) => println!("  Create failed (record may not exist): {e}"),
        }

        println!("\nUI API example complete.");
        Ok(())
    }
}

#[cfg(feature = "ui")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    example::run().await
}

#[cfg(not(feature = "ui"))]
fn main() {
    eprintln!("Run with --features ui");
}
