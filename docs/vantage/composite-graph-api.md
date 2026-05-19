# 🔭 Vantage: Spec for Composite Graph API

**What business problem does this solve?**
Salesforce restricts the standard Composite API to 25 subrequests. Enterprise customers managing complex transactional hierarchies (e.g., creating an Account, multiple Contacts, and associated Opportunities in one go) hit this limit frequently, forcing them to manually chunk requests and write complex rollback logic if a later chunk fails. The Composite Graph API allows up to 500 subrequests grouped into transactional "graphs", ensuring cross-record references and atomic rollback at the graph level, saving massive developer overhead and reducing API quota usage.

👤 **User Story:**
As an Integration Developer, I want to execute up to 500 dependent DML operations in a single atomic transaction using a dependency graph, so that I don't have to build custom batching and rollback logic for deep object hierarchies.

✅ **Acceptance Criteria:**
- **Graph Construction:** Must provide a builder to easily add nodes and establish reference dependencies (e.g., node B depends on node A's `@{ReferenceId.id}`).
- **Graph Serialization:** Must correctly serialize the graph structure to the Salesforce Composite Graph API format.
- **Transactionality:** Must support `allOrNone` behavior at the graph level to guarantee atomic rollbacks.
- **Response Parsing:** Must correctly parse the complex, nested response structure, mapping individual subrequest results back to their original reference IDs.

🚫 **Out of Scope:**
- Automatic client-side limit detection and splitting (if a user tries to send 501 nodes, the API returns an error; we will just pass it through for now).
- Local query validation against org metadata before sending.

📊 **Metric Definition:**
- Success = Ability to successfully insert a 50-node deep hierarchy in a single API call with all cross-references correctly resolved.

🔍 **Gap Analysis:**
- **Current State:** The library supports standard Composite API (25 limit), but users must manually chain requests for larger transactions, risking partial failures.
- **Market Standard:** Competing SDKs either ignore Graph API or expose it purely as a raw JSON passthrough. Providing a strongly-typed graph builder gives us a massive DX advantage over raw HTTP clients.
