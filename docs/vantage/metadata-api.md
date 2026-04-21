# 🔭 Vantage: Spec for Metadata API Handler

**What business problem does this solve?**
Salesforce developers, release managers, and DevOps teams need to programmatically deploy and retrieve complex declarative metadata (e.g., Custom Objects, Profiles, Layouts) across environments. Manual package creation is tedious, and existing REST/Tooling APIs do not scale for full environment backups or large, multi-component ZIP-based deployments. We need a robust API surface to interact with Salesforce's SOAP-based Metadata API.

**Gap Analysis:**
Currently, `force-rs` provides extensive data manipulation capabilities (REST, Bulk, Composite) and limited Tooling API operations. However, it lacks a standard method for packaging, deploying, or retrieving full `.zip` based metadata components. Downstream tooling (like CLI deployers or backup systems) must currently stitch together their own SOAP handlers, which is complex and error-prone. Standard libraries in other ecosystems (like `jsforce` or `salesforcex`) have first-class Metadata API support.

**Success Metric:**
Success = Ability to seamlessly trigger a full `retrieve` or `deploy` of a 50MB metadata zip file, handle asynchronous status polling intuitively without manual loop orchestration, and surface structured deployment errors for CI/CD environments.

👤 **User Story:**
As a DevOps Engineer, I want to programmatically retrieve and deploy metadata zip files using a clean Rust API, so that I can automate environment backups and CI/CD releases without dealing with raw SOAP payloads or manual polling loops.

✅ **Acceptance Criteria:**
- Must provide a `metadata` feature flag that exposes a `MetadataHandler` interface.
- Must implement `deploy()` and `retrieve()` operations, handling the underlying SOAP XML payload conversion transparently.
- Must support zipping and unzipping payloads efficiently.
- Must include an async `poll_status()` mechanism to check the progress of long-running deployment/retrieval jobs until completion or failure.
- Must return structured error types that capture specific Salesforce metadata deployment errors (e.g., component-level compilation failures) to aid debugging.

🚫 **Out of Scope:**
- Automatic conflict resolution for merged XML files.
- Parsing or modifying individual metadata XML files inside the ZIP (this feature only focuses on the transport and job lifecycle).
- Real-time synchronous deployments (the Salesforce Metadata API is inherently asynchronous).
