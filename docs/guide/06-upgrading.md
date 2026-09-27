# Upgrading

Guide for upgrading between `force` releases: what breaks and the minimal
before/after fix, so upgrading is a search-and-replace instead of an
archaeology dig through [CHANGELOG.md](../../CHANGELOG.md).

## 0.3 → 0.4

0.4.0 made three semver-breaking API-shape corrections, all found by live-org
contract testing: the old shapes compiled but could not deserialize a real
org's response. If your code accessed any of these three directly, upgrading
breaks the build until you apply the matching fix below.

### 1. `OrgLimits` fields are now `Option`-wrapped

Real orgs omit some limits from the response (not every edition returns
`DailyBatchApexExecutions`, for example), so every named field on `OrgLimits`
moved from a bare `LimitInfo` to `Option<LimitInfo>`.

Before (0.3), accessed directly:

```rust,ignore
let limits = client.rest().limits().await?;
println!("{}/{}", limits.daily_api_requests.remaining, limits.daily_api_requests.max);
```

After (0.4), the field must be unwrapped or matched:

<!-- onramp-fragment: upgrade_org_limits -->
```rust
let limits = client.rest().limits().await?;
if let Some(daily) = &limits.daily_api_requests {
    println!("{}/{}", daily.remaining, daily.max);
}
```

### 2. `RecordDefaultsRepresentation.object_info` → `object_infos`

The UI API's record-defaults payload actually returns a map of object infos
(one per referenced object on the layout), not a single value. The field was
renamed to its real, plural shape.

Before (0.3), a single value:

```rust,ignore
let defaults = client.ui().create_defaults("Account").await?;
let account_info = defaults.object_info;
```

After (0.4), look up by API name in the map:

<!-- onramp-fragment: upgrade_object_infos -->
```rust
let defaults = client.ui().create_defaults("Account").await?;
let account_info = defaults.object_infos.get("Account");
```

### 3. GraphQL `query_raw` now returns `Err` on GraphQL errors

Previously, a response carrying `errors` alongside null-filled `data` came
back as `Ok(Value::Null)`-shaped data — the `?` operator never caught
anything, and callers had to inspect the raw JSON themselves to notice a
partial failure. `query_raw` now surfaces those errors as
`Err(ForceError::GraphQL(_))`, matching what `query`/`query_with_errors`
already did.

Before (0.3), a partial failure was silent:

```rust,ignore
let data = client.graphql().query_raw(query, None).await?;
// `data` could be null-filled here even though the request partially
// failed -- nothing above ever returned `Err`.
```

After (0.4), the same `?` now catches it:

<!-- onramp-fragment: upgrade_query_raw -->
```rust
let data = client.graphql().query_raw(query, None).await?;
// If the response carried GraphQL errors, this line never runs: the `?`
// above already returned `Err(ForceError::GraphQL(_))`.
println!("{data}");
```

## See also

- [CHANGELOG.md](../../CHANGELOG.md) — full per-release change list.
- [API Stability and SemVer Policy](../governance/api-stability-policy.md)
- [Release and Versioning](05-release-and-versioning.md)
