# Runbook: Auth Credential Rotation

## Purpose

Rotate Salesforce authentication material (client secret, JWT private key, access token source) without service outage.

## Scope

- OAuth client credentials
- JWT bearer auth keys
- CI/live-contract secrets (`SF_ACCESS_TOKEN`, `SF_INSTANCE_URL`)

## Preconditions

- New credential/key is issued and validated in Salesforce.
- Rollback credential is still valid.
- Deployment channel is available.

## Procedure

1. Stage new secret in secret manager as `*_NEXT`.
2. Deploy app version that can read both active and next secrets.
3. Switch active reference to new secret.
4. Restart or roll pods/instances.
5. Verify:
   - auth success rate
   - token refresh success
   - no spike in `401 INVALID_SESSION_ID`
6. Keep previous secret for rollback window (24-72h).
7. Revoke old credential in Salesforce after window closes.

## Verification Commands

```bash
cargo test -p force --all-features --test live_salesforce -- --ignored --test-threads=1
```

Run only after setting valid `SF_ACCESS_TOKEN` and `SF_INSTANCE_URL`.

## Rollback

1. Restore old secret reference.
2. Restart services.
3. Verify auth success and error rates return to baseline.

## Common Failure Signals

- Persistent `401` after rotation
- `invalid_client` during token exchange
- JWT signature validation failures

## Escalation

- If auth outage exceeds 10 minutes, trigger incident and rollback immediately.

