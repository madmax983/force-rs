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

The following diagram illustrates the internal module structure and dependencies. Authentication state and logic are encapsulated in the `Auth` module.

```mermaid
C4Component
  title Component Diagram for Force SDK

  Container_Boundary(sdk, "Force SDK") {
    Component(client, "Client Core", "crates/force/client", "Facade & Configuration")
    Component(api, "API Handlers", "crates/force/api", "REST, Bulk, Composite logic")
    Component(auth, "Auth", "crates/force/auth", "Authentication, Token Persistence & State")
    Component(http, "HTTP", "crates/force/http", "Resilience & Middleware")

    Rel(client, api, "Exposes")
    Rel(api, client, "Uses (Session State)")
    Rel(client, auth, "Uses (TokenManager)")
    Rel(client, http, "Uses (HttpExecutor)")
    Rel(api, http, "Uses (HttpExecutor)")
  }
```

## Class Diagram

The core library uses a `Session` struct pattern for shared state and thread safety, decoupled from storage via the `TokenManager`.

```mermaid
classDiagram
  class ForceClient {
    +rest() RestHandler
    +bulk() BulkHandler
  }
  class Session {
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

  ForceClient *-- Session : Shared State (Arc)
  RestHandler *-- Session : Shared State (Arc)
  Session --> TokenManager : Owns
  Session --> HttpExecutor : Owns
  TokenManager --> Authenticator : Uses (Strategy)
```

## Sequence Diagram: Token Storage

The following sequence diagram illustrates how the Client interacts with the TokenManager to retrieve and persist authentication tokens.

```mermaid
sequenceDiagram
    participant C as Client (ForceClient)
    participant S as Session
    participant TM as TokenManager
    participant A as Authenticator

    Note over C, TM: Token Retrieval Flow
    C->>S: token()
    S->>TM: token()
    TM->>TM: Check Internal Cache (Read Lock)
    alt Valid Token Exists
        TM-->>S: AccessToken
        S-->>C: AccessToken
    else Expired / None
        TM->>TM: Acquire Write Lock
        TM->>A: refresh() or authenticate()
        alt Success
            A-->>TM: New AccessToken
            TM->>TM: Update Cache
            TM-->>S: New AccessToken
            S-->>C: New AccessToken
        else Failure
            A-->>TM: Error
            TM-->>S: Error
            S-->>C: Error
        end
    end
```

## C4 Component Diagram: Bulk API & Smart Ingest

The Bulk API module includes a high-level utility `SmartIngest` that orchestrates the complex lifecycle of bulk jobs.

```mermaid
C4Component
  title Component Diagram for Bulk API & Smart Ingest

  Container_Boundary(bulk_mod, "Bulk Module") {
    Component(smart_ingest, "SmartIngest", "High-Level Utility", "Orchestrates streaming upload & polling")
    Component(bulk_handler, "BulkHandler", "API Facade", "Manages Job Lifecycle (Create, AddBatch, Close)")
    Component(csv_ser, "CSV Serializer", "csv crate", "Serializes Rust structs to CSV")

    Rel(smart_ingest, csv_ser, "Uses")
    Rel(smart_ingest, bulk_handler, "Calls")
  }

  System_Ext(sf_bulk, "Salesforce Bulk API 2.0")

  Rel(bulk_handler, sf_bulk, "HTTP/REST")
```

## Sequence Diagram: Smart Ingest Lifecycle

The following sequence diagram illustrates the `SmartIngest::execute_stream` workflow, handling chunking, uploading, and polling.

```mermaid
sequenceDiagram
    participant App
    participant SI as SmartIngest
    participant BH as BulkHandler
    participant SF as Salesforce API

    App->>SI: execute_stream(records)
    activate SI
    SI->>BH: create_job()
    BH->>SF: POST /jobs/ingest
    SF-->>BH: Job ID (Open)
    BH-->>SI: Job ID

    loop Every Batch (10k records)
        SI->>SI: Buffer & Serialize CSV
        SI->>BH: upload_batch()
        BH->>SF: PUT /jobs/ingest/.../batches
        SF-->>BH: 201 Created
    end

    SI->>BH: close_job()
    BH->>SF: PATCH /jobs/ingest/... (UploadComplete)
    SF-->>BH: 200 OK

    loop Polling
        SI->>BH: get_job()
        BH->>SF: GET /jobs/ingest/...
        SF-->>BH: Job Status
        alt JobComplete
            SI-->>App: JobInfo
        else Failed/Aborted
            SI-->>App: Error
        end
    end
    deactivate SI
```

## C4 Component Diagram: HTTP Layer

The HTTP layer is decomposed into specialized modules for execution, retry logic, and observability.

```mermaid
C4Component
  title Component Diagram for HTTP Layer

  Container_Boundary(http_mod, "HTTP Module") {
     Component(executor, "Executor", "executor.rs", "Request execution & Middleware orchestration")
     Component(retry, "Retry Logic", "retry.rs", "Backoff & idempotency policies")
     Component(telemetry, "Telemetry", "telemetry.rs", "Observability hooks & tracing context")
     Component(error, "Error Handling", "error.rs", "Response parsing & error mapping")

     Rel(executor, retry, "Uses")
     Rel(executor, telemetry, "Uses")
     Rel(executor, error, "Uses")
  }
```
