# 🔭 Vantage: Spec for Apex REST API

**Business Problem ("So What?"):**
Salesforce customers often write custom server-side logic using Apex, exposing it via custom REST endpoints (e.g., `/services/apexrest/MyCustomEndpoint/`). Currently, developers using the `force` crate have to manually construct HTTP requests and manage authentication to call these endpoints. We need a native, type-safe abstraction to invoke custom Apex REST services seamlessly, reducing boilerplate and preventing endpoint typos or manual serialization bugs.

**Gap Analysis:**
The standard REST API (`rest` feature) handles basic CRUD operations well but doesn't map nicely to custom Apex REST URIs. Standard HTTP libraries (like raw `reqwest`) require developers to manually inject bearer tokens and handle Salesforce's custom JSON error responses. We lack an `apex_rest()` handler on the `ForceClient` that handles this domain-specific problem out-of-the-box.

👤 **User Story:**
As an Enterprise Integration Developer, I want to execute custom Apex REST endpoints natively using the `force` client, so that I can invoke complex, transaction-safe custom Salesforce logic without manually writing HTTP boilerplate or managing auth tokens.

✅ **Acceptance Criteria:**
- Must provide an `.apex_rest()` builder or handler on `ForceClient`.
- Must support `GET`, `POST`, `PUT`, `PATCH`, and `DELETE` HTTP methods on the custom endpoint.
- Must automatically append `/services/apexrest/` to the requested path.
- Must handle strongly-typed serialization of the request body and deserialization of the response via `serde`.
- Success = A developer can call an Apex endpoint in < 3 lines of Rust code with full type safety for inputs and outputs.

🚫 **Out of Scope:**
- Parsing or deploying the actual Apex code to the server (Tooling API handles deployments).
- Calling custom SOAP services (only custom REST is supported).
