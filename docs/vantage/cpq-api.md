# CPQ API

## User Story
As a Sales Representative or Revenue Operations Engineer, I want to programmatically interact with Salesforce CPQ (Configure, Price, Quote) via the ServiceRouter API, so that I can automate quote generation, product configuration, pricing calculations, document generation, and contract amendments without manual UI intervention.

## So What?
Salesforce CPQ operations often require complex, multi-step REST calls through the opaque `SBQQ/ServiceRouter` endpoint. By providing a typed, programmatic interface, we reduce the friction of integrating quote-to-cash workflows into external systems, enabling faster and more accurate sales cycles.

## Metric Definition
Success = Ability to successfully complete a full Quote-to-Cash API lifecycle (Read Quote, Add Products, Calculate Pricing, Save Quote, Generate Document) via the wrapper with < 1% error rate due to malformed API requests.

## Gap Analysis
Currently, standard Salesforce REST APIs do not cover the intricate logic of CPQ, which relies on the custom `SBQQ/ServiceRouter` Apex REST endpoint. Existing standard HTTP clients require manual JSON payload crafting and tracking of complex CPQ models. There is a gap in the ecosystem for a strongly-typed Rust client to handle these operations out-of-the-box.

## Acceptance Criteria
- Must support reading and saving quotes.
- Must support product configuration and adding products to a quote.
- Must support triggering the CPQ pricing calculation engine.
- Must support document generation for quotes.
- Must support contract amendments and renewals.

## Out of Scope
- Building a UI for CPQ configuration.
- Syncing CPQ data to local databases (handled by sync engine).
