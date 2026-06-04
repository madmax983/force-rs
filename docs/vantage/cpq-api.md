# 🔭 Vantage: Spec for CPQ API Support

**Business problem:**
Salesforce CPQ (Configure, Price, Quote) data is heavily locked behind the ServiceRouter Apex REST endpoints. Customers need to automate quote calculations, contract amendments, and product configurations without manual UI clicks, which currently requires brittle, undocumented REST calls.

**Gap Analysis:**
Currently, developers must manually construct ServiceRouter JSON payloads and handle custom loader paths, leading to runtime errors and difficult-to-maintain code.

**Success metric:**
Success = Ability to read a quote, add products, calculate pricing, and save the quote entirely via strongly-typed Rust methods without manually crafting JSON payloads.

👤 **User Story:**
As a Revenue Operations Engineer, I want to programmatically interact with Salesforce CPQ, so that I can automate contract amendments and quote generation as part of our order-to-cash pipeline.

✅ **Acceptance Criteria:**
- Must provide strongly-typed models for Quotes, Quote Lines, and Products.
- Must support reading, calculating, saving quotes, and adding products via the ServiceRouter.
- Must support contract amendments.
- Must handle the double-serialized JSON envelope required by the CPQ ServiceRouter.

🚫 **Out of Scope:**
- Full CPQ pricing rule engine emulation locally.
- Non-quote related operations like Advanced Approvals.
