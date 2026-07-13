# Agentforce (Models + Agent API)

Two Einstein AI Platform surfaces:

- **Models API** — the Einstein LLM gateway: text generation, chat, and embeddings.
- **Agent API** — headless Agentforce agent sessions (start / message / end).

| | Feature flag | Accessor |
|---|---|---|
| Models | `models` | `client.models()` → `ModelsHandler` |
| Agent | `agent_api` | `client.agents()` → `AgentHandler` |

The umbrella feature `agentforce` enables both.

## Host and authentication

Both handlers target the **fixed host `https://api.salesforce.com`** with a version-less
path — *not* the org `instance_url`. Override per-handler with `.with_host(...)` for
Government Cloud (`https://api.gov.salesforce.com`) or testing.

They reuse the org's standard OAuth bearer token (no token exchange), **but the backing
OAuth client must be registered as an External Client App** (not a classic connected app)
with the Einstein/Models platform scopes enabled. See
[choosing an auth flow](../02-choosing-an-auth-flow.md).

Models requests additionally carry two required headers, `x-sfdc-app-context` and
`x-client-feature-id`, injected automatically.

## Models

```rust
use force::api::models::{GenerateTextRequest, ChatMessage, ChatGenerationRequest, ModelName};

let models = client.models();

// Text generation.
let resp = models
    .generate_text(
        ModelName::DEFAULT_GPT4_OMNI,
        &GenerateTextRequest::new("Invent 3 fun names for donuts"),
    )
    .await?;
println!("{:?}", resp.text());

// Chat.
let chat = models
    .generate_chat(
        ModelName::DEFAULT_GPT4_OMNI,
        &ChatGenerationRequest::new(vec![ChatMessage::user("Give me a pie recipe")]),
    )
    .await?;
println!("{:?}", chat.reply_text());
```

| Method | Purpose |
|---|---|
| `generate_text(model, &req)` | Single-prompt text generation |
| `generate_chat(model, &req)` | Multi-turn chat completion |
| `generate_embeddings(model, &req)` | Embedding vectors |

`ModelName` is an open newtype with constants like `DEFAULT_GPT4_OMNI`,
`DEFAULT_GPT4_OMNI_MINI`, and `DEFAULT_OPENAI_TEXT_EMBEDDING_ADA_002`; typing across the
Trust Layer is deliberately permissive.

## Agent

```rust
use force::api::agent_api::SessionEndReason;

let agents = client.agents();

let session = agents.start_session_default("0XxHr000000ysOSKAY").await?;
let reply = agents
    .send_text(&session.session_id, 1, "How do I reset my password?")
    .await?;
println!("{:?}", reply.reply_text());
agents.end_session(&session.session_id, SessionEndReason::UserRequest).await?;
```

| Method | Purpose |
|---|---|
| `start_session(agent_id, &req)` | Start a session with an explicit `StartSessionRequest` |
| `start_session_default(agent_id)` | Start with sensible defaults (generated key + My Domain endpoint) |
| `send_message(session_id, &req)` | Send a structured `SendMessageRequest` |
| `send_text(session_id, sequence_id, text)` | Send a plain-text turn |
| `end_session(session_id, reason)` | End the session (sends `x-session-end-reason` header) |

`end_session` carries the `SessionEndReason` as the `x-session-end-reason` header.
`AgentResponse::reply_text()` pulls the agent's text out of the polymorphic response.

**Streaming:** only the synchronous message endpoint is implemented; the streaming SSE
endpoint (`.../messages/stream`) is a documented follow-up and out of scope today.

## See also

- [ADR-028 — Agentforce Models + Agent API design](../../adr/028-agentforce-api-design.md)
- Rustdoc: `force::api::models` (`ModelsHandler`, `ModelName`, `GenerateTextRequest`) and `force::api::agent_api` (`AgentHandler`, `SessionEndReason`, `AgentResponse`)
