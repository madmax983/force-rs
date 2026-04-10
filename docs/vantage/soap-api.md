# 🔭 Vantage: Spec for SOAP API

**Business Problem ("So What?"):**
While Salesforce strongly encourages using the REST API or Bulk API for most modern integrations, there remains a massive ecosystem of legacy enterprise integrations, on-premise systems, and specific third-party tools that rely exclusively on the Salesforce Enterprise or Partner SOAP APIs. Without SOAP support, the `force` crate is unusable for these enterprise customers, forcing them to maintain fragmented codebases (using `force` for REST, and another tool/library for SOAP). Supporting the SOAP API unlocks migration paths for older enterprise integrations.

**Gap Analysis:**
The `force` crate currently supports modern API surfaces (REST, Bulk V2, Composite, GraphQL) but has no XML/SOAP serialization or deserialization capabilities. Rust has a different ecosystem for XML compared to JSON (`serde_xml_rs` or `quick-xml` instead of `serde_json`). We lack a `soap()` handler and the necessary tooling to generate Rust types from the Enterprise WSDL or interact with the Partner WSDL dynamically.

👤 **User Story:**
As an Enterprise Systems Integrator, I want to execute Salesforce SOAP API calls (using the Partner or Enterprise WSDL) through the `force` crate, so that I can maintain and gradually modernize legacy integrations without needing a separate Java or C# service just to talk to the SOAP API.

✅ **Acceptance Criteria:**
- Must provide a `.soap()` builder or handler on `ForceClient`.
- Must support XML serialization/deserialization for SOAP envelopes.
- Must implement the core SOAP operations: `login()`, `query()`, `create()`, `update()`, `delete()`, `upsert()`.
- Must support passing a session ID obtained via standard `Authenticator` (OAuth) to the SOAP header, avoiding the need to use the legacy SOAP `login()` if modern auth is available.
- Success = A developer can execute a basic SOAP `query` or `create` using Rust structs that map to the XML schema.

🚫 **Out of Scope:**
- Automatic runtime WSDL parsing (we assume users will use pre-generated code or standard dynamic structures).
- Supporting every obscure SOAP-only feature (e.g., Lead Conversion via SOAP) in the first phase; focus on core CRUD.
