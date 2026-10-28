# API Catalog (MCP server registry)

Register external MCP servers in the org and manage their tools, prompts, and
resources. Agentforce can then use these assets. New in Winter '27 (v68.0).
This handler manages registry records. It is not an MCP client.

- **Feature flag:** `api_catalog`
- **Handler accessor:** `client.api_catalog()` → `ApiCatalogHandler`
- **Base path:** `/services/data/vXX.X/api-catalog/mcp-servers`

## Register and fetch

```rust
use force::api::api_catalog::{McpServerAuthorizationInput, McpServerCreateInput};

let input = McpServerCreateInput::external("weather", "https://mcp.example.com/mcp")
    .with_authorization(McpServerAuthorizationInput::no_auth());
let created = client.api_catalog().create_mcp_server(&input).await?;

// Connect to the server again and read its current assets.
let fetched = client.api_catalog().fetch_mcp_server(&created.server.id).await?;
for asset in &fetched.assets {
    println!("{} {:?} {:?}", asset.name, asset.kind, asset.security_warning);
}
```

`McpServerAuthorizationInput::oauth(client_id, client_secret)` keeps the secret
out of `Debug` output.

| Method | HTTP | Purpose |
|---|---|---|
| `list_mcp_servers(&opts)` | `GET /mcp-servers` | List servers (filter by label, type, `ACTIVE`/`DISCONNECTED`) |
| `create_mcp_server(&input)` | `POST /mcp-servers` | Register a server; returns the first asset fetch |
| `get_mcp_server(id)` | `GET /mcp-servers/{id}` | Get one server |
| `update_mcp_server(id, &input)` | `PUT /mcp-servers/{id}` | Change label, description, URL, or auth |
| `delete_mcp_server(id)` | `DELETE /mcp-servers/{id}` | Remove a server |
| `fetch_mcp_server(id)` | `POST /mcp-servers/{id}/fetch` | Read assets from the live server |
| `list_mcp_server_assets(id)` | `GET /mcp-servers/{id}/assets` | List saved assets |
| `replace_mcp_server_assets(id, &input)` | `PUT /mcp-servers/{id}/assets` | Replace the saved asset list |

Unknown enum values from the server decode to `Unknown`. IDs must be
alphanumeric; the handler sends no request for a bad ID.

## See also

- [ADR-035 — Winter '27 (v68.0) adoption](../../adr/035-winter-27-v68-adoption.md)
- Rustdoc: `cargo doc --no-deps --features api_catalog --open` → `force::api::api_catalog`
