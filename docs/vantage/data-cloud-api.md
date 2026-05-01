# 🔭 Vantage: Spec for Data Cloud API

**What business problem does this solve?**
Salesforce Data Cloud (formerly CDP) operates on a completely distinct tenant architecture from core platform APIs. Customers need to query their unified customer profiles and calculated insights, but currently have to manually juggle two distinct authentication lifecycles: obtaining a core OAuth token, then performing a specialized POST exchange to get a short-lived Data Cloud token, and then managing requests against an entirely different endpoint format. This creates error-prone authentication logic and disjointed client usage across pipelines. A unified API surface abstracts the tenant complexity, allowing users to query Data Cloud insights just like any other Salesforce object.

👤 **User Story:**
As a Data Engineer, I want to query Data Cloud using the standard client interface, so that I don't have to manually build the custom token exchange logic, tenant URL resolution, or manage separate retry loops and connection pools.

✅ **Acceptance Criteria:**
- **Seamless Token Exchange:** Must automatically intercept standard Salesforce platform tokens and execute the required two-step token exchange against the token endpoint to retrieve the Data Cloud tenant URL and specialized access token.
- **Independent Auth Lifecycle:** The Data Cloud token must maintain its own independent soft and hard expiration windows without interfering with the main platform token cache.
- **Universal Authenticator:** The solution must ensure any core auth flow (JWT, OAuth, etc.) can seamlessly power Data Cloud access without requiring the user to orchestrate the handoff.
- **Unified Client Interface:** The SDK must expose a Data Cloud handler, allowing queries against `ssot/` endpoints exactly like the standard REST handler.
- **Shared Connection Infrastructure:** Must reuse the existing HTTP client and connection pools to minimize overhead, rather than instantiating a completely isolated HTTP stack.
- **Failure Resilience:** Token exchange failures must trigger a clear "Data Cloud token exchange failed" error.
- **Opt-in Only:** The feature must be completely opt-in via a configuration flag, returning a configuration error if the handler is invoked without configuring the client appropriately.

🚫 **Out of Scope:**
- Creating a completely separate client instance for Data Cloud (Users should not have to manage two discrete client instances).
- Modifying the core underlying session or HTTP request implementation logic.
- Building custom pagination or query-builder utilities specifically for Data Cloud (this relies on the standard JSON and REST query features).

📊 **Metric Definition:**
- Success = 100% of standard platform auth flows (JWT, Client Credentials, etc.) can be seamlessly reused to query Data Cloud without writing custom token exchange logic.
- Performance = Token exchange caching ensures $<10\text{ms}$ overhead on Data Cloud requests after the initial token fetch.

🔍 **Gap Analysis:**
- **Current State:** Developers using the library must drop down to raw HTTP clients and build custom authentication loops to interact with Data Cloud, completely abandoning the safety and ergonomics of the SDK.
- **Market Standard:** Other enterprise Salesforce SDKs (like JSForce or the official Java SDK) often abstract tenant-specific endpoint routing, but typically struggle to elegantly unify the auth lifecycles. By providing a unified abstraction, we can offer a best-in-class, zero-overhead developer experience.
