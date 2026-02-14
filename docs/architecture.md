# Architecture Documentation

This document describes the high-level architecture of the Force SDK.

## C4 Context Diagram

```mermaid
C4Context
  title System Context Diagram for Force SDK

  Person(developer, "Developer", "A user of the Force SDK")
  System(force_sdk, "Force SDK", "Rust SDK for Salesforce")
  System_Ext(salesforce, "Salesforce API", "The external Salesforce platform")

  Rel(developer, force_sdk, "Uses")
  Rel(force_sdk, salesforce, "Make API calls")
```

## Internal Architecture

The `force` crate is organized into modular components.

```mermaid
graph TB
    subgraph "force crate"
        Client[Client Layer]
        Auth[Auth Layer]
        Storage[Storage Layer]
        HTTP[HTTP Layer]
    end

    Client --> Storage
    Storage --> Auth
    Storage --> HTTP
```

## Sequence Diagram: Token Storage

The following sequence diagram illustrates how the Client interacts with the Storage layer (TokenManager) to persist authentication tokens.

```mermaid
sequenceDiagram
    participant C as Client (force::client)
    participant S as Storage (force::storage)
    participant A as Auth (force::auth)

    C->>S: token()
    alt Token Cached
        S-->>C: AccessToken
    else Token Expired/Missing
        S->>A: authenticate()
        A-->>S: AccessToken
        S->>S: update_cache()
        S-->>C: AccessToken
    end
```
