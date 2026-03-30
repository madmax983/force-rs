# 🔭 Vantage: Spec for SOAP API

**What business problem does this solve?**
While REST and Bulk APIs are the modern standard, many legacy enterprise systems, particularly on-premise ERPs and older integrations, rely exclusively on WSDL-based SOAP interfaces. Without supporting the Salesforce Enterprise and Partner SOAP APIs, these customers are unable to migrate to `force-rs` without completely rewriting their integration middleware.

👤 **User Story:**
As an Enterprise Systems Architect, I want to execute strongly-typed XML-based SOAP requests against Salesforce, so that my legacy integrations can interact with Salesforce without adopting the newer REST interfaces.

✅ **Acceptance Criteria:**
- Must establish authenticated SOAP connections using standard XML envelope headers.
- Must support standard CRUD operations and arbitrary SOAP method execution.
- Must provide error parsing from standard SOAP Fault envelopes.
- Success = Ability to execute a SOAP `login` or `query` operation and parse the XML response back into Rust structs with < 200ms latency.

🚫 **Out of Scope:**
- Automatic WSDL-to-Rust code generation (Phase 2 - use generic XML payloads for Phase 1).
- Complex nested SOAP types (focus on Partner API semantics first).
