# Data Anonymizer

## The 'So What?'
When cloning production data to developer sandboxes, companies often expose sensitive Personally Identifiable Information (PII) to developers and third-party contractors, violating privacy compliance. The Data Anonymizer solves this by systematically masking or synthesizing sensitive fields during the sandbox seeding process.

## User Story
As a Compliance Officer, I want to automatically mask PII data when migrating records to a sandbox, so that development teams can test with realistic data without violating data privacy regulations.

## Metric Definition
- Success = 100% of fields tagged as 'PII' in the data dictionary are overwritten with synthetic or masked data before insertion into the target sandbox.
- Performance = Can process and anonymize 10,000 records per minute.

## Gap Analysis
Currently, teams rely on custom post-refresh scripts that are error-prone and often fail due to governor limits. Standard tools only do static replacements. The market lacks an integrated solution that works natively during the data loading phase and generates realistic fake data to preserve data shapes.

## Acceptance Criteria
- Must support applying specific masking rules based on field configuration.
- Must preserve relationships and reference integrity.
- Must log a detailed audit report of which objects and fields were anonymized.
- Must seamlessly integrate with the existing data seeding workflows.
- Must handle missing data or empty fields without failing.

## Out of Scope
- Discovering or auto-classifying PII data based on field content.
- Anonymizing data in production environments.
