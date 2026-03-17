# Runbook: Retry and Polling Tuning

## Purpose

Tune request retries and bulk polling to improve resilience without overloading Salesforce or duplicating writes.

## Safety Model

- `Read`: safe to retry.
- `IdempotentMutation`: safe to retry when business semantics are idempotent.
- `Mutation`: default conservative retries.

Use explicit idempotent APIs for write paths where replay is safe.

## Tuning Knobs

### HTTP retries

- `read_max_retries`
- `idempotent_mutation_max_retries`
- `mutation_max_retries`

### Bulk polling

- `BulkPollPolicy::max_attempts`
- `BulkPollPolicy::initial_backoff`
- `BulkPollPolicy::max_backoff`

## Tuning Workflow

1. Start from defaults.
2. Change one knob at a time.
3. Validate with staging and synthetic failure scenarios.
4. Watch:
   - success rate
   - p95/p99 latency
   - `429` rate
   - duplicate-write risk indicators
5. Roll forward only if all guardrails remain healthy.

## Anti-Patterns

- Retrying non-idempotent writes aggressively.
- Tight polling loops for bulk jobs.
- Global retry increases without endpoint segmentation.

