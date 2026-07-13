# ADR-028: Agentforce API Design

**Date:** 2026-07-12
**Status:** Accepted
**Feature flags:** `models`, `agent_api`, `agentforce`

## Context

Agentforce is Salesforce's generative-AI platform. Two distinct REST surfaces are relevant to a headless Rust client:

- **Models API** — a direct LLM gateway (`/einstein/platform/v1/`) for single-turn text generation, multi-turn chat, and embeddings against Salesforce-hosted models (`sfdc_ai__DefaultGPT4Omni`, provider-backed models, etc.).
- **Agent API** — a conversational orchestration surface (`/einstein/ai-agent/v1/`) that drives a configured Agentforce agent: start a session, exchange messages, end the session.

Both diverge from every existing handler in one structural way: they target the **fixed host `https://api.salesforce.com`** with a **version-less path**, not the org `instance_url` used by `Session::resolve_url()`. The Agent API additionally needs the org's My Domain URL, but passes it **in the request body** (`instanceConfig.endpoint`) rather than using it as the request host.

## Decision

### 1. Two feature flags plus an `agentforce` umbrella

The two APIs ship under independent flags — `models` and `agent_api` — because they are genuinely separable: a caller wanting only the LLM gateway should not pull in the Agent API (and its `uuid` dependency), and vice versa. An `agentforce = ["models", "agent_api"]` umbrella feature is provided for callers who want the whole surface, mirroring how `full`/`all` aggregate the other API flags. Both `models` and `agent_api` are added to `full` so the meta-feature stays comprehensive.

Only `agent_api` pulls a new dependency (`uuid`, for generating `externalSessionKey`), gated as `agent_api = ["dep:uuid"]`.

### 2. `api.salesforce.com` host divergence and the `with_host` override

Rather than route through `Session::resolve_url()` (which prepends `instance_url` + `/services/data/{version}`), each handler stores its own `host: String` field, defaulting to the `AI_PLATFORM_HOST` constant (`https://api.salesforce.com`), and builds absolute URLs directly (e.g. `format!("{host}/einstein/platform/v1/models/{model}/generations")`).

A consuming builder `with_host(self, impl Into<String>) -> Self` overrides the host. This serves two purposes:

1. **Government Cloud** — callers on `api.gov.salesforce.com` swap the host in one call.
2. **Testing** — wiremock tests point the handler at the mock server's URI, exactly like the mock-server pattern used by every other handler, without needing a special resolver.

`Clone` is hand-implemented on both handlers to clone both the `Arc<Session>` and the `host` string. Token auth is still injected downstream by `Session::execute_request`, so the handlers never set `Authorization` themselves — only the two special Models headers and the Agent end-reason header are set explicitly.

### 3. Reused `ClientCredentials`, but an External Client App is required

Neither API needs a token-exchange decorator (contrast ADR-022's Data Cloud design). The existing `Session`/`TokenManager` and any `Authenticator` (typically `ClientCredentials`) produce a standard bearer token that both APIs accept as-is. The **wire flow is unchanged**.

The one operational difference — documented in the handler-level doc comments — is that the backing OAuth client must be registered as a Salesforce **External Client App** (not a classic connected app) with the Einstein/Models platform scopes enabled, and the agent must be linked to that app. This is a registration/configuration concern, not a code-path difference, so it is surfaced in docs rather than encoded in types.

### 4. Permissive, optional response typing for Trust Layer fields

Response bodies carry Trust Layer and provider-specific data whose shape varies by model provider and evolves over time: `contentQuality`/`scanToxicity`, generation `parameters`, `result`, `citedReferences`, and HATEOAS `_links`. These are modeled as `Option<serde_json::Value>` (or `Vec<Value>` with `#[serde(default)]`), and nearly every response field is `Option<T>`. Convenience accessors (`GenerateTextResponse::text`, `ChatGenerationResponse::reply_text`, `AgentResponse::reply_text`) extract the common happy-path values without forcing callers to navigate the permissive shapes.

The Agent API `messages[]` array is **polymorphic** — its `type` field (`Inform`, `ProgressIndicator`, `Error`, streaming chunk types, …) drives the shape. `AgentMessage.message_type` is therefore an open `String`, never a closed enum, so future/streaming message types deserialize into the same struct. For the same reason `ModelName` is an open `String` newtype with associated `&str` constants for common models, not an enum.

### 5. Synchronous only; streaming is a documented follow-up

The Agent API exposes a streaming SSE endpoint (`.../messages/stream`). It is intentionally out of scope for this iteration — it requires an SSE-capable byte-stream response path and eventsource parsing. Only the synchronous `messages` endpoint is implemented; a module-level doc comment records streaming as a follow-up. Keeping `AgentMessage.message_type` an open `String` means the existing types already accommodate streaming event variants when that work lands. The Models `feedback` endpoint is likewise deferred.

### 6. The special Models headers

Every Models request must carry two exact headers in addition to `Authorization`/`Content-Type`:

- `x-sfdc-app-context: EinsteinGPT`
- `x-client-feature-id: ai-platform-models-connected-app`

These are applied centrally by a private `with_models_headers` helper so all three Models methods stay consistent, and their presence is asserted in the wiremock tests via `header(...)` matchers. The Agent API's `end_session` similarly sets the required `x-session-end-reason` header, driven by the `SessionEndReason` enum (default `UserRequest`).

## Consequences

- **Positive:** Callers get typed, ergonomic access to the Agentforce LLM gateway and headless agents through the same `ForceClient` entry point (`client.models()` / `client.agents()`), with Gov Cloud and test-host overrides in one call. Permissive typing insulates callers from Trust Layer shape churn.
- **Positive:** No new auth machinery; the APIs ride the existing token pipeline.
- **Negative / follow-ups:** Streaming responses, the Models `feedback` endpoint, and richer typed models for `result`/`citedReferences` remain unimplemented. The External Client App requirement is enforced by Salesforce configuration, not by the type system, so misconfiguration surfaces at runtime as an auth error rather than a compile error.
