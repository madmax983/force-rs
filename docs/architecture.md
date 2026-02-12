# System Architecture

This document describes the high-level architecture of `force-rs`.

## Class Diagram

The following class diagram illustrates the decoupled relationship between the Core logic and the Storage mechanism.

```mermaid
classDiagram
  class Core
  class Storage
  Core --> Storage : Uses (Trait Bound)
  %% Removed the circular dependency arrow
```

## Sequence Diagram

The following sequence diagram illustrates the flow of data persistence.

```mermaid
sequenceDiagram
    participant Core
    participant Storage
    Core->>Storage: Save Data
    Storage-->>Core: Confirm Success
```
