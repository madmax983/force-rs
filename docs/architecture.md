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

## C4 Component Diagram

The following diagram illustrates the internal module structure and dependencies. Note that the `Storage` module is decoupled from the core `Client` logic.

```mermaid
C4Component
  title Component Diagram for Force SDK

  Container_Boundary(sdk, "Force SDK") {
    Component(client, "Client Core", "crates/force/client", "Facade & Configuration")
    Component(api, "API Handlers", "crates/force/api", "REST, Bulk, Composite logic")
    Component(storage, "Storage", "crates/force/storage", "Token persistence & state")
    Component(auth, "Auth", "crates/force/auth", "Authentication strategies")
    Component(http, "HTTP", "crates/force/http", "Resilience & Middleware")

    Rel(client, api, "Exposes")
    Rel(api, client, "Uses (Inner State)")
    Rel(client, storage, "Uses (TokenManager)")
    Rel(client, http, "Uses (HttpExecutor)")
    Rel(storage, auth, "Uses (Authenticator)")
    Rel(api, http, "Uses (HttpExecutor)")
  }
```

## Class Diagram

The core library uses an `Inner` struct pattern for shared state and thread safety, decoupled from storage via the `TokenManager`.

```mermaid
classDiagram
  class ForceClient {
    +rest() RestHandler
    +bulk() BulkHandler
  }
  class Inner {
    +execute_request()
  }
  class TokenManager {
    +token() AccessToken
  }
  class Authenticator {
    <<interface>>
    +authenticate()
    +refresh()
  }
  class RestHandler

  ForceClient *-- Inner : Shared State (Arc)
  RestHandler *-- Inner : Shared State (Arc)
  Inner --> TokenManager : Owns
  Inner --> HttpExecutor : Owns
  TokenManager --> Authenticator : Uses (Strategy)
```

## Sequence Diagram: Token Storage

The following sequence diagram illustrates how the Client interacts with the TokenManager to retrieve and persist authentication tokens.

```mermaid
sequenceDiagram
    participant C as Client (ForceClient)
    participant I as Inner
    participant TM as TokenManager
    participant A as Authenticator

    Note over C, TM: Token Retrieval Flow
    C->>I: token()
    I->>TM: token()
    TM->>TM: Check Internal Cache (Read Lock)
    alt Valid Token Exists
        TM-->>I: AccessToken
        I-->>C: AccessToken
    else Expired / None
        TM->>TM: Acquire Write Lock
        TM->>A: refresh() or authenticate()
        alt Success
            A-->>TM: New AccessToken
            TM->>TM: Update Cache
            TM-->>I: New AccessToken
            I-->>C: New AccessToken
        else Failure
            A-->>TM: Error
            TM-->>I: Error
            I-->>C: Error
        end
    end
```

## Sequence Diagram: Smart Ingest

The `SmartIngest` utility automates the Bulk API 2.0 lifecycle, including chunking, uploading, and polling.

```mermaid
sequenceDiagram
    participant App
    participant SI as SmartIngest
    participant BH as BulkHandler
    participant SF as Salesforce API

    App->>SI: execute_stream(records)
    SI->>BH: create_job()
    BH->>SF: POST /jobs/ingest
    SF-->>BH: Job ID (Open)
    BH-->>SI: Job ID

    loop Every Batch (10k records)
        SI->>SI: Buffer & Serialize (CSV)
        SI->>BH: upload_batch(job_id, csv)
        BH->>SF: PUT /jobs/ingest/{id}/batches
        SF-->>BH: 201 Created
    end

    SI->>BH: close_job(job_id)
    BH->>SF: PATCH /jobs/ingest/{id} (UploadComplete)
    SF-->>BH: Job Info (UploadComplete)

    loop Polling
        SI->>BH: get_job(job_id)
        BH->>SF: GET /jobs/ingest/{id}
        SF-->>BH: Job Info (InProgress)
        SI->>SI: sleep(backoff)
    end

    SF-->>BH: Job Info (JobComplete)
    BH-->>SI: Job Info
    SI-->>App: Job Info
```
