//! UI API favorites endpoints.
//!
//! Provides CRUD operations for user favorites (pinned items) via the
//! Salesforce UI API (`/services/data/vXX.0/ui-api/favorites`).

#![allow(clippy::doc_markdown)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ─── Response types ──────────────────────────────────────────────────────────

/// The full collection of user favorites.
///
/// Returned by `get_favorites()`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoritesRepresentation {
    /// The list of favorited items.
    pub favorites: Vec<FavoriteRepresentation>,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A single favorited item.
///
/// Returned by `get_favorites()`, `create_favorite()`, and `update_favorite()`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteRepresentation {
    /// The server-assigned ID for this favorite.
    pub id: String,
    /// Human-readable label shown in the UI.
    pub label: String,
    /// Optional name (may be different from label).
    pub name: Option<String>,
    /// The target identifier (e.g., a record ID or URL).
    pub target: Option<String>,
    /// The target type (e.g., `"Record"`, `"List"`).
    pub target_type: Option<String>,
    /// Whether the item is currently saved as a favorite.
    pub saved: bool,
    /// Any additional fields returned by the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ─── Input types ─────────────────────────────────────────────────────────────

/// Input body for creating or updating a favorite.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteInput {
    /// Human-readable label to display in the UI.
    pub label: String,
    /// Optional name for the favorite.
    pub name: Option<String>,
    /// The target identifier (e.g., a record ID or URL).
    pub target: String,
    /// The target type (e.g., `"Record"`, `"List"`).
    pub target_type: String,
}

// ─── UiHandler<A> implementation ─────────────────────────────────────────────

impl<A: crate::auth::Authenticator> crate::api::ui::UiHandler<A> {
    /// Returns all favorites for the current user.
    ///
    /// Calls `GET /ui-api/favorites`.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn get_favorites(&self) -> crate::error::Result<FavoritesRepresentation> {
        self.get("favorites", None, "Failed to fetch favorites")
            .await
    }

    /// Creates a new favorite for the current user.
    ///
    /// Calls `POST /ui-api/favorites`.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is invalid or the request fails.
    pub async fn create_favorite(
        &self,
        input: &FavoriteInput,
    ) -> crate::error::Result<FavoriteRepresentation> {
        self.post("favorites", input, "Failed to create favorite")
            .await
    }

    /// Updates an existing favorite.
    ///
    /// Calls `PATCH /ui-api/favorites/{id}`.
    ///
    /// * `id` – The server-assigned favorite ID.
    /// * `input` – Updated label, name, target, and target type.
    ///
    /// # Errors
    ///
    /// Returns an error if the favorite is not found or the request fails.
    pub async fn update_favorite(
        &self,
        id: &str,
        input: &FavoriteInput,
    ) -> crate::error::Result<FavoriteRepresentation> {
        let path = format!("favorites/{id}");
        self.patch(&path, input, "Failed to update favorite").await
    }

    /// Deletes a favorite.
    ///
    /// Calls `DELETE /ui-api/favorites/{id}`. Returns `()` on success (204).
    ///
    /// * `id` – The server-assigned favorite ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the favorite is not found or deletion fails.
    pub async fn delete_favorite(&self, id: &str) -> crate::error::Result<()> {
        let path = format!("favorites/{id}");
        self.delete_empty(&path, "Failed to delete favorite").await
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {

    use super::*;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const FAV_ID: &str = "0MV000000000001AAA";

    async fn make_client(server: &MockServer) -> crate::client::ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &server.uri());
        builder().authenticate(auth).build().await.must()
    }

    fn favorite_json(id: &str, label: &str) -> serde_json::Value {
        json!({
            "id": id,
            "label": label,
            "name": null,
            "target": "001000000000001AAA",
            "targetType": "Record",
            "saved": true
        })
    }

    fn make_input() -> FavoriteInput {
        FavoriteInput {
            label: "My Account".to_string(),
            name: None,
            target: "001000000000001AAA".to_string(),
            target_type: "Record".to_string(),
        }
    }

    // ── get_favorites success with items ──────────────────────────────────────

    #[tokio::test]
    async fn test_get_favorites_success_with_items() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({
            "favorites": [
                favorite_json(FAV_ID, "My Account"),
                favorite_json("0MV000000000002AAA", "My Contact")
            ]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/ui-api/favorites"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().get_favorites().await.must();

        assert_eq!(result.favorites.len(), 2);
        assert_eq!(result.favorites[0].id, FAV_ID);
        assert_eq!(result.favorites[0].label, "My Account");
        assert!(result.favorites[0].saved);
    }

    // ── get_favorites empty list ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_favorites_empty_list() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let response_body = json!({ "favorites": [] });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/ui-api/favorites"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().get_favorites().await.must();
        assert!(result.favorites.is_empty());
    }

    // ── create_favorite success ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_favorite_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/ui-api/favorites"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(favorite_json(FAV_ID, "My Account")),
            )
            .expect(1)
            .mount(&server)
            .await;

        let input = make_input();
        let result = client.ui().create_favorite(&input).await.must();

        assert_eq!(result.id, FAV_ID);
        assert_eq!(result.label, "My Account");
        assert_eq!(result.target_type.as_deref(), Some("Record"));
    }

    // ── create_favorite 400 error ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_favorite_bad_request() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/ui-api/favorites"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
                "errorCode": "INVALID_INPUT",
                "message": "target is required"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let input = FavoriteInput {
            label: "Bad".to_string(),
            name: None,
            target: String::new(),
            target_type: "Record".to_string(),
        };

        let result = client.ui().create_favorite(&input).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── update_favorite success ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_favorite_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        let updated = favorite_json(FAV_ID, "Renamed Account");

        Mock::given(method("PATCH"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/favorites/{FAV_ID}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&updated))
            .expect(1)
            .mount(&server)
            .await;

        let input = FavoriteInput {
            label: "Renamed Account".to_string(),
            name: None,
            target: "001000000000001AAA".to_string(),
            target_type: "Record".to_string(),
        };

        let result = client.ui().update_favorite(FAV_ID, &input).await.must();
        assert_eq!(result.id, FAV_ID);
        assert_eq!(result.label, "Renamed Account");
    }

    // ── update_favorite 404 ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_favorite_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("PATCH"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/favorites/{FAV_ID}"
            )))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "Favorite not found"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().update_favorite(FAV_ID, &make_input()).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── delete_favorite success (204) ─────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_favorite_success() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("DELETE"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/favorites/{FAV_ID}"
            )))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        client.ui().delete_favorite(FAV_ID).await.must();
    }

    // ── delete_favorite 404 ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_favorite_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server).await;

        Mock::given(method("DELETE"))
            .and(path(format!(
                "/services/data/v60.0/ui-api/favorites/{FAV_ID}"
            )))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!([{
                "errorCode": "NOT_FOUND",
                "message": "Favorite not found"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let result = client.ui().delete_favorite(FAV_ID).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── unit: FavoriteInput serialization ────────────────────────────────────

    #[test]
    fn test_favorite_input_serialization() {
        let input = FavoriteInput {
            label: "Test Favorite".to_string(),
            name: Some("test_fav".to_string()),
            target: "001000000000001AAA".to_string(),
            target_type: "Record".to_string(),
        };

        let serialized = serde_json::to_value(&input).must();
        assert_eq!(serialized["label"], "Test Favorite");
        assert_eq!(serialized["name"], "test_fav");
        assert_eq!(serialized["target"], "001000000000001AAA");
        assert_eq!(serialized["targetType"], "Record");
    }
}
