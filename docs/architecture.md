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

## Class Diagram

The core library is decoupled from concrete storage implementations via traits.

```mermaid
classDiagram
  class Core
  class Storage
  Core --> Storage : Uses (Trait Bound)
  %% Removed the circular dependency arrow
```

## Sequence Diagram: Token Storage

The following sequence diagram illustrates how the Core interacts with the Storage layer to persist authentication tokens.

```mermaid
sequenceDiagram
    participant C as Core (force)
    participant S as Storage (force-storage)

    Note over C, S: Authentication Flow
    C->>C: Authenticate with Salesforce
    C->>S: save_token(access_token)
    alt Success
        S-->>C: Ok(())
    else Failure
        S-->>C: Err(StorageError)
        C->>C: Log error (graceful degradation)
    end
```
