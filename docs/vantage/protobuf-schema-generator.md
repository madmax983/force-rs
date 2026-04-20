# 🔭 Vantage: Spec for Protobuf Schema Generator

**What business problem does this solve?**
Enterprise organizations with microservices architectures often standardize on gRPC and Protocol Buffers (proto3) for high-performance, strongly-typed internal communication. When these systems need to process Salesforce data (e.g., streaming events or bulk extracts), developers spend significant time manually translating Salesforce's complex object metadata (`SObjectDescribe`) into `.proto` definitions. This manual process is slow, error-prone, and rapidly becomes stale when the Salesforce schema changes.

**Gap Analysis:**
Currently, teams either maintain these `.proto` files by hand, risk runtime errors due to mismatched types, or write custom one-off scripts. There is a need for an automated capability that bridges the Salesforce metadata API with standard proto3 schema definitions natively.

👤 **User Story:**
As a Backend Engineer building gRPC microservices, I want to automatically generate `.proto` message definitions from Salesforce object metadata, so that I can establish strongly-typed, high-performance data contracts without manually maintaining schema translations.

✅ **Acceptance Criteria:**
- Must correctly map standard Salesforce field types to corresponding proto3 scalar types (e.g., `string` to `string`, `double` to `double`, `boolean` to `bool`).
- Must handle optionality properly, generating `optional` fields where Salesforce fields are nullable.
- Must generate standard proto3 syntax files including proper `syntax = "proto3";` headers and package names.
- Success = Ability to generate valid, compileable `.proto` schema files for a complex object like `Account` or `Contact` in under 1 second, directly from `SObjectDescribe` payloads.

🚫 **Out of Scope:**
- Compiling the generated `.proto` files into Rust, Go, or Java code (this is left to standard tools like `protoc` or `prost`).
- Managing or hosting a schema registry.
- Generating the actual gRPC service definitions; only message payloads are in scope.
