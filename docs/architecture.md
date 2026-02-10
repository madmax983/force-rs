# Architecture Documentation

This document describes the high-level architecture of the `force-rs` library.

## C4 System Context

The following diagram illustrates the system context for `force-rs`.

```mermaid
C4Context
    title System Context Diagram for force-rs

    Person(user, "Developer", "A developer building an application that integrates with Salesforce.")
    System(force_rs, "force-rs", "A Rust library for interacting with the Salesforce REST and Bulk APIs.")
    System_Ext(salesforce, "Salesforce Platform", "The Salesforce cloud platform providing CRM data and APIs.")

    Rel(user, force_rs, "Uses", "Rust Code")
    Rel(force_rs, salesforce, "Makes API Requests", "HTTPS/JSON/CSV")
```

## Component Diagram

The following class diagram illustrates the core components of the library, focusing on the HTTP execution layer.

```mermaid
classDiagram
    class ForceClient {
        +new(instance_url, version, authenticator)
        +query(soql)
        +create(sobject, data)
        +update(sobject, id, data)
        +delete(sobject, id)
    }

    class HttpExecutor {
        -client: reqwest::Client
        -retry_policy: RetryPolicy
        -telemetry_hooks: TelemetryHooks
        +execute(request)
        +execute_json(request)
    }

    class Authenticator {
        <<interface>>
        +token()
    }

    class RetryPolicy {
        +read_max_retries
        +mutation_max_retries
    }

    class TelemetryHooks {
        +on_retry
        +on_complete
    }

    ForceClient --> HttpExecutor : Uses
    ForceClient --> Authenticator : Uses
    HttpExecutor --> RetryPolicy : Configured by
    HttpExecutor --> TelemetryHooks : Emits events to
```

## Sequence Diagram: Request Retry Logic

The following sequence diagram illustrates how the `HttpExecutor` handles transient failures like rate limits and authentication expiration.

```mermaid
sequenceDiagram
    participant User
    participant ForceClient
    participant HttpExecutor
    participant Salesforce

    User->>ForceClient: query("SELECT Id FROM Account")
    ForceClient->>HttpExecutor: execute(request)

    loop Retry Loop
        HttpExecutor->>Salesforce: HTTP Request (Token A)

        alt 429 Too Many Requests
            Salesforce-->>HttpExecutor: 429 (Retry-After: 2)
            HttpExecutor->>HttpExecutor: Sleep(2s)
            note right of HttpExecutor: Retry after delay
        else 503 Service Unavailable
            Salesforce-->>HttpExecutor: 503
            HttpExecutor->>HttpExecutor: Exponential Backoff
        else 401 Unauthorized
            Salesforce-->>HttpExecutor: 401
            HttpExecutor->>HttpExecutor: Refresh Token
            note right of HttpExecutor: New Token B
        else 200 OK
            Salesforce-->>HttpExecutor: 200 OK (JSON)
            HttpExecutor-->>ForceClient: Response
            break
        end
    end

    ForceClient-->>User: Result<Vec<Record>>
```
