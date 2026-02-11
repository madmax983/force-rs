# ForceRS Architecture

This document describes the high-level architecture of the `force-rs` crate, including the decoupled storage module.

## System Context (C4 Level 1)

```mermaid
C4Context
  title System Context: ForceRS

  Person(user, "User", "A developer using the force-rs crate")
  System(force_rs, "ForceRS", "A Rust client for the Salesforce Platform API")
  System_Ext(salesforce, "Salesforce Platform", "The target CRM system")
  System_Ext(database, "Database", "Persistence Layer (e.g. SQLite, Redis)")

  Rel(user, force_rs, "Uses")
  Rel(force_rs, salesforce, "Makes API calls to")
  Rel(force_rs, database, "Persists state/cache to")
```

## Container Diagram (C4 Level 2)

```mermaid
C4Container
  title Container Diagram: ForceRS Crate Structure

  Container_Boundary(force_rs, "ForceRS Workspace") {
    Component(core, "Core", "Rust", "Authentication, Config, HTTP Executor, Domain Logic")
    Component(rest, "REST API", "Rust", "SOQL, CRUD, Search")
    Component(bulk, "Bulk API 2.0", "Rust", "Large volume data operations (CSV)")
    Component(storage, "Storage", "Rust", "Persistence Logic (Decoupled)")
  }

  System_Ext(salesforce, "Salesforce Platform", "REST/Bulk APIs")
  System_Ext(database, "Database", "File/DB")

  Rel(core, salesforce, "HTTP/1.1 (Auth)")
  Rel(rest, core, "Uses")
  Rel(bulk, core, "Uses")
  Rel(core, storage, "Uses (Trait Bound)")
  Rel(storage, database, "Reads/Writes")
```

## Component Design (Classes)

```mermaid
classDiagram
  class Core {
    +ForceClient
    +Inner
  }
  class Storage {
    <<interface>>
    +save_token()
    +load_token()
  }
  class StorageImpl {
    +save_token()
    +load_token()
  }
  class RestHandler
  class BulkHandler

  Core --> Storage : Uses (Trait Bound)
  StorageImpl ..|> Storage : Implements
  RestHandler --> Core
  BulkHandler --> Core
```

## Storage Sequence Flow

```mermaid
sequenceDiagram
  participant Core
  participant Storage
  participant Database

  Core->>Storage: save_token(token)
  Storage->>Database: INSERT INTO tokens ...
  Database-->>Storage: Success
  Storage-->>Core: Ok(())
```
