# 07 — Troubleshooting first-run errors

Error messages from `force` link here. Each row: the message fragment, the cause, what to do.

| Message contains | Cause | Fix |
|---|---|---|
| `invalid_client_id` / `invalid_client` | Wrong consumer key or secret, or the Connected App has not propagated yet (can take a few minutes). | In Setup > App Manager > your app > *Manage Consumer Details*, copy the key and secret again. |
| `no client credentials user` | Client Credentials flow is enabled but no *Run As* user is set. | Setup > App Manager > your app > *Manage* > *Edit Policies* > Client Credentials Flow > set **Run As**. |
| `invalid_grant` | Flow not enabled for the app, or credentials/user/login URL belong to a different org (production vs sandbox vs My Domain). | Enable the flow on the Connected App and make sure `Environment`/token URL match the org. See [02 — Choosing an Auth Flow](02-choosing-an-auth-flow.md). |
| `API_DISABLED_FOR_ORG` | The org edition has no API access or the user lacks **API Enabled**. | Use an edition with API access (Developer Edition is free) and grant the permission to the profile/permission set. |
| `MALFORMED_QUERY` | SOQL syntax error or unknown field/object. | Check the query against the object's fields (`describe`). |
| `NOT_FOUND` (404) | Wrong sObject/record name or an API version the org does not support. | Check the API name (custom objects end in `__c`) and the configured API version. |
