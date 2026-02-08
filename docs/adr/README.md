# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records (ADRs) for force-rs. ADRs document significant architectural and design decisions made during the development of the project.

## What is an ADR?

An Architecture Decision Record (ADR) captures a single architectural decision and its context. Each ADR describes:
- The context and problem statement
- The decision drivers and constraints
- Options considered
- The chosen decision and rationale
- Consequences (positive, negative, neutral)
- Related decisions

## Format

ADRs follow this structure:
1. **Title** - Decision number and descriptive name
2. **Status** - Proposed, Accepted, Deprecated, Superseded
3. **Date** - When the decision was made
4. **Deciders** - Who made the decision
5. **Context** - Problem statement and background
6. **Decision Drivers** - Constraints and requirements
7. **Considered Options** - Alternatives evaluated
8. **Decision Outcome** - What was chosen and why
9. **Consequences** - Positive/negative/neutral impacts
10. **Validation** - How success will be measured
11. **Related Decisions** - Links to other ADRs
12. **References** - External documentation

## Index

### Foundation Phase

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [001](001-workspace-structure.md) | Workspace Structure and Module Organization | Accepted | 2026-02-07 |
| [002](002-authentication-strategy.md) | Authentication Strategy and Trait Design | Accepted | 2026-02-07 |
| [003](003-error-handling.md) | Error Handling Strategy with thiserror | Accepted | 2026-02-07 |
| [004](004-feature-gates.md) | Feature Flag Strategy for API Surfaces | Accepted | 2026-02-07 |
| [005](005-compile-time-auth-safety.md) | Compile-Time Auth Safety with Phantom Types | Accepted | 2026-02-07 |
| [006](006-handler-pattern.md) | Handler Pattern for API Operations | Accepted | 2026-02-07 |

### Future ADRs

Planned ADRs for upcoming decisions:
- **ADR-007**: HTTP Client and Middleware Architecture
- **ADR-008**: Rate Limiting and Retry Strategy
- **ADR-009**: Token Storage and Caching
- **ADR-010**: SOQL Query Builder Design
- **ADR-011**: Bulk API Job Management
- **ADR-012**: Pub/Sub gRPC Integration
- **ADR-013**: Testing Strategy and Mock Framework
- **ADR-014**: Logging and Observability
- **ADR-015**: Connection Pooling Configuration
- **ADR-016**: API Versioning Strategy

## Decision Process

When making significant architectural decisions:

1. **Identify the decision** - What needs to be decided?
2. **Research options** - What are the alternatives?
3. **Evaluate tradeoffs** - Pros/cons of each option
4. **Make decision** - Choose and document rationale
5. **Create ADR** - Document in this directory
6. **Review** - Get team feedback
7. **Accept** - Mark as accepted and implement
8. **Validate** - Verify assumptions through implementation

## Relationships Between ADRs

```mermaid
graph TD
    ADR001[ADR-001: Workspace Structure] --> ADR002[ADR-002: Authentication]
    ADR001 --> ADR003[ADR-003: Error Handling]
    ADR001 --> ADR004[ADR-004: Feature Gates]
    ADR001 --> ADR006[ADR-006: Handler Pattern]

    ADR002 --> ADR003
    ADR002 --> ADR004
    ADR002 --> ADR005[ADR-005: Compile-Time Auth Safety]

    ADR004 --> ADR006
    ADR006 --> ADR002

    ADR002 -.-> ADR007[ADR-007: HTTP Middleware]
    ADR003 -.-> ADR007
    ADR004 -.-> ADR007

    ADR002 -.-> ADR009[ADR-009: Token Storage]
    ADR007 -.-> ADR008[ADR-008: Retry Strategy]

    style ADR001 fill:#4a9eff
    style ADR002 fill:#ffd43b
    style ADR003 fill:#ff6b6b
    style ADR004 fill:#51cf66
    style ADR005 fill:#a78bfa
    style ADR006 fill:#f472b6
    style ADR007 fill:#ddd
    style ADR008 fill:#ddd
    style ADR009 fill:#ddd
```

## Key Decisions Summary

### ADR-001: Workspace Structure
- **Decision**: Use Cargo workspace with single `force` library crate in `crates/force/`
- **Rationale**: Ready for future expansion while keeping initial complexity low
- **Impact**: Clear module boundaries, workspace-level lints

### ADR-002: Authentication Strategy
- **Decision**: Trait-based `Authenticator` with generic `ForceClient<Auth>`
- **Rationale**: Compile-time auth safety, zero-cost abstraction, extensible
- **Impact**: Can't create unauthenticated clients, type-safe auth flows

### ADR-003: Error Handling
- **Decision**: Hierarchical error types with `thiserror`
- **Rationale**: Type-safe error handling, clear error context for callers
- **Impact**: Users can match on specific errors, `?` operator works seamlessly

### ADR-004: Feature Gates
- **Decision**: Fine-grained features for API surfaces (default: `rest`)
- **Rationale**: Zero-cost abstraction, only compile what's needed
- **Impact**: Fast compile times, small binaries, opt-in dependencies

### ADR-005: Compile-Time Auth Safety
- **Decision**: Phantom type state pattern for builder
- **Rationale**: Prevent unauthenticated clients at compile time
- **Impact**: `ForceClientBuilder<State>` with type transitions, zero runtime cost

### ADR-006: Handler Pattern
- **Decision**: Lightweight handler objects for API operations (`client.rest()`, `client.bulk()`)
- **Rationale**: Clear namespacing, feature isolation, organized documentation
- **Impact**: Slightly longer calls but better organization and discoverability

## Contributing ADRs

When adding a new ADR:

1. Use the next sequential number (e.g., `005-title.md`)
2. Follow the standard ADR template
3. Include mermaid diagrams for architecture
4. Link related ADRs bidirectionally
5. Update this README index
6. Get team review before marking as "Accepted"

## Status Meanings

- **Proposed** - Under discussion, not yet decided
- **Accepted** - Decision made and documented
- **Deprecated** - No longer applicable
- **Superseded** - Replaced by another ADR (link to successor)

## References

- [ADR Template](https://github.com/joelparkerhenderson/architecture-decision-record)
- [Why ADRs](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
- [Mark's Coding Standards](C:\Users\markm\.claude\CLAUDE.md) - "ADRs for significant decisions"
