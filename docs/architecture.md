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

The core library is decoupled from concrete storage implementations via module boundaries and strategy patterns.

```mermaid
classDiagram
  class ForceClient
  class TokenManager
  class Authenticator
  <<interface>> Authenticator
  ForceClient --> TokenManager : Uses (Module Import)
  TokenManager --> Authenticator : Uses (Strategy Pattern)
  %% Circular dependency between Core and Storage is resolved
```

## Sequence Diagram: Token Storage

The following sequence diagram illustrates how the Client interacts with the TokenManager to retrieve and persist authentication tokens.

```mermaid
sequenceDiagram
    participant C as Client (ForceClient)
    participant TM as TokenManager
    participant A as Authenticator

    Note over C, TM: Token Retrieval Flow
    C->>TM: token()
    TM->>TM: Check Internal Cache (Read Lock)
    alt Valid Token Exists
        TM-->>C: AccessToken
    else Expired / None
        TM->>TM: Acquire Write Lock
        TM->>A: refresh() or authenticate()
        alt Success
            A-->>TM: New AccessToken
            TM->>TM: Update Cache
            TM-->>C: New AccessToken
        else Failure
            A-->>TM: Error
            TM-->>C: Error
        end
    end
```
