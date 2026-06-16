# Data Masking Utility Spec

## User Story
As a Developer or QA Engineer, I want to mask sensitive data (PII, financial records) in full-copy or partial-copy sandboxes, so that we can safely use production-like data for testing without violating compliance regulations (e.g., GDPR, HIPAA).

## Acceptance Criteria
- Must provide configurable masking strategies (e.g., scramble, anonymize, randomize, static value replacement).
- Must operate on large datasets via the Bulk API 2.0 to ensure performance.
- Must ensure deterministic masking for related records (e.g., same email mapped to the same fake email).
- Must output a masking audit report for compliance verification.

## Out of Scope
- Real-time on-the-fly data masking during REST API retrieval (Phase 2).
