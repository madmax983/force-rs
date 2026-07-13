# Account Engagement (Pardot) API v5

Typed access to the Account Engagement (formerly Pardot) v5 object model: prospects, lists,
list memberships, campaigns, custom fields, forms, and emails.

- **Feature flag:** `account_engagement`
- **Handler accessor:** `client.account_engagement(business_unit_id)` → `AccountEngagementHandler`

## Important differences from the core APIs

- **Separate host.** Requests do *not* go to the org `instance_url`. They target
  `https://pi.pardot.com` (production/training) or `https://pi.demo.pardot.com`
  (sandbox/demo), derived from the client `Environment`. Override with `.with_host(...)`.
- **Business-unit header is required.** The `0Uv…` business unit id you pass to
  `account_engagement(...)` is sent on every request as the `Pardot-Business-Unit-Id`
  header. The connected app must also carry the `pardot_api` OAuth scope.
- **`fields` is mandatory.** Query/read endpoints return no object data unless you pass a
  `fields` list (comma-separated). This is the single most common cause of "empty" responses.
- **Integer IDs.** v5 object IDs are integers, not the 15/18-char Salesforce IDs.

## Example

```rust
let ae = client
    .account_engagement("0Uv000000000001AAA")
    .with_host("https://pi.demo.pardot.com"); // optional override

// `fields` is REQUIRED to return any data.
let page = ae
    .query_prospects("id,email,firstName,lastName", &[("limit", "50")])
    .await?;
for prospect in page.values {
    println!("{:?}", prospect.email);
}

let one = ae.get_prospect("42", "id,email,score").await?;
```

`query_*` methods return `QueryResponse<T>` — a `{ values, next_page_token, next_page_url }`
pagination envelope. On the follow-up page, pass only `fields` + `nextPageToken`; both cursor
fields are absent on the final page.

Prospects have full CRUD (`query_prospects`, `get_prospect`, `create_prospect`,
`update_prospect`, `delete_prospect`); lists, list memberships, campaigns, custom fields,
forms, and emails each expose their own typed methods (see their modules).

## Escape hatch

For objects not yet modeled, use the raw helpers (path relative to `/api/v5/`, BU header
applied automatically):

```rust
let visits = ae.get_raw("objects/visits", Some(&[("fields", "id,prospectId")])).await?;
```

`get_raw` / `post_raw` / `patch_raw` / `delete_raw` return/accept `serde_json::Value`. API
errors surface as `ForceError::Http`.

## See also

- [ADR-029 — Account Engagement API v5 separate-host design](../../adr/029-account-engagement-api-design.md)
- Rustdoc: `force::api::account_engagement` (`AccountEngagementHandler`, `QueryResponse`, `Prospect`, `List`, `Campaign`)
