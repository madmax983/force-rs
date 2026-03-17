# Runbook: Rate-Limit Incident (`429` / `REQUEST_LIMIT_EXCEEDED`)

## Purpose

Restore service when Salesforce API limits are exceeded or near exhaustion.

## Detection

- Elevated `HttpError::RateLimitExceeded`
- Elevated `HTTP 429`
- Salesforce payload contains `REQUEST_LIMIT_EXCEEDED`

## Immediate Mitigation

1. Reduce outbound concurrency.
2. Enable backpressure for non-critical workloads.
3. Increase retry backoff for reads and idempotent writes.
4. Pause bulk/non-urgent jobs.

## Triage Checklist

1. Confirm affected endpoints (REST query, CRUD, Bulk polling/results).
2. Identify traffic source (batch jobs, deploy, replay, user spike).
3. Confirm if retries are amplifying load.
4. Check org limits via REST limits endpoint.

## Recommended Runtime Actions

- Prefer retrying read/idempotent traffic only.
- Keep non-idempotent mutation retries conservative.
- Increase polling intervals for bulk jobs.
- Drain queue before re-enabling normal rates.

## Post-Incident Actions

1. Capture peak request rate and duration.
2. Add/update alert thresholds.
3. Tune retry and poll policy defaults.
4. Document capacity assumptions and expected envelopes.

