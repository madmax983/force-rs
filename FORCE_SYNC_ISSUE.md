# 🔭 Vantage: Spec for force-sync Engine

**What business problem does this solve?**
Enterprise teams need a durable, resilient way to synchronize data bidirectionally between Salesforce and PostgreSQL. Relying solely on raw API clients forces developers to reinvent complex control planes (task leasing, reconciliation, conflict resolution, deduplication) for every integration project. A native sync engine reduces this integration boilerplate, guarantees data correctness during transient failures, and provides a warehouse-friendly operational history.

👤 **User Story:**
As a Data Engineer, I want a durable, Postgres-first sync engine integrated with the `force` ecosystem, so that I can reliably orchestrate bidirectional data synchronization between Salesforce and my local database without manually building task queues, journals, or conflict resolution logic.

✅ **Acceptance Criteria:**
- Must use PostgreSQL as the v0.1 durable backend for the sync control plane (storing journals, links, tasks, checkpoints, conflicts).
- Must use external IDs (e.g., `External_Id__c`) as the canonical identity across systems, treating Salesforce IDs as aliases.
- Must implement a planner-driven transport architecture that automatically selects the cheapest safe apply lane (REST, Composite Graph, or Bulk) based on workload size.
- Must be decoupled from the core `force` crate to avoid bloating API-only consumers.
- Success = Ability to seamlessly resume from a crash and repair drift using an external-ID based upsert strategy without data loss or duplication.

🚫 **Out of Scope:**
- SQLite backend support (deferred to later phases).
- Direct data warehouse sinks (enterprises should load from the Postgres journal downstream).
