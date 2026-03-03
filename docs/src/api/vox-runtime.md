# Crate API: vox-runtime

## Overview

The Vox runtime provides the execution infrastructure for compiled Vox applications, built on **Tokio** and **Axum**.

## Architecture

### Actor System

Every `actor` declaration in Vox compiles to an isolated Tokio task with a message-passing mailbox:

- **ProcessHandle**: A lightweight handle for sending messages to an actor (`send`, `call`)
- **ProcessRegistry**: A global registry mapping actor IDs to their handles
- **Mailbox**: An `mpsc` channel providing asynchronous message delivery with configurable capacity

### Scheduler

The scheduler manages lifecycle and message routing:

- Spawns actor tasks with proper Tokio supervision
- Routes messages between actors via the registry
- Supports request/reply patterns through `oneshot` channels

### Subscription Manager

Reactive subscriptions for real-time data updates:

- Table-level change notifications
- Client subscription tracking
- SSE (Server-Sent Events) integration for push-based updates (see `StreamTransport` in `transport.rs`; WebSocket reserved for future use)

### Local Storage

Content-addressable file storage:

- Deterministic IDs for identical content
- Automatic deduplication via BLAKE3 or XXH3 (see [Builtins: Hashing](../ref-builtins-hashing.md))
- URL-based access via `/storage/{id}` prefix

## Key Design Decisions

1. **No `null`**: All optional values use `Option<T>`, following Vox's zero-null philosophy
2. **Lock safety**: All `RwLock`/`Mutex` operations use `.expect()` with descriptive messages instead of `.unwrap()`
3. **Content addressing**: Storage uses content hashing (FNV-1a) instead of timestamp-based IDs for deterministic behavior

---

### `struct ActivityOptions`

Options that control activity execution behavior.
These map directly to the `with { ... }` syntax in Vox source.


### `enum ActivityResult`

Result of an activity execution.


### `enum ActivityError`

Error from activity execution.


### `struct ActivityExecution`

Tracks the state of an activity execution for observability.


### `fn execute_activity`

Execute an async activity function with the given options.

This is the core runtime function that compiled `with { ... }` expressions
call into. It handles retries, timeouts, and exponential backoff.


### `struct AuthConfig`

Authentication configuration used by generated Vox services.


### `fn authorize_request`

Validate an incoming API key and/or bearer token.


### `fn parse_bearer_token`

Parse `Authorization: Bearer <token>`.


### `fn parse_retry_after_seconds`

Parse an optional header duration used by throttling.


### `fn get_db`

Initialize the global database instance using environment variables.

Looks for:
- `VOX_DB_PATH`: Local SQLite file path (default: `vox_state/vox.db`)
- `TURSO_URL`: Remote Turso URL
- `TURSO_AUTH_TOKEN`: Remote Turso auth token


### `fn ensure_schema`

Ensure the database schema matches the provided digest.

This should be called during application startup (usually in the generated `main()`).


## Module: `vox-runtime\src\feedback.rs`

# Feedback Collector

Logs LLM interactions and user feedback to the Vox database for
RLHF training data collection and quality monitoring.

```no_run
use vox_runtime::feedback::FeedbackCollector;
use vox_pm::store::CodeStore;

async fn example(store: CodeStore) {
let collector = FeedbackCollector::new(store, "session-1", Some("alice".to_string()));
let id = collector.log("What is Vox?", "Vox is an AI-native language.", 42, 150).await.unwrap();
collector.thumbs_up(id).await.unwrap();
}
```


### `struct FeedbackCollector`

Collects LLM interactions and feedback for RLHF training.


## Module: `vox-runtime\src\lib.rs`

# vox-runtime

Execution runtime for compiled Vox applications, built on Tokio and warp.

Provides the actor system, scheduler, subscription manager, and local
content-addressable storage that compiled Vox programs rely on.


### `struct ChatMessage`

Message format for the chat API


### `struct LlmConfig`

A configuration block for an LLM provider integration.


### `struct ModelRegistryEntry`

An entry in a Vox `@config model_registry:` block, deserialized at compile time.


### `struct ModelMetric`

Tracks token usage and cost per LLM call — stored in @table ModelMetric.
Serializable so it can be persisted to VoxDB directly.


### `struct LlmResponse`

The standard parsed response from an LLM chat operation.


### `fn llm_chat`

Core durable wrapper for LLM chat (single complete response).


### `fn llm_stream`

Token-by-token streaming implementation.


### `enum Envelope`

A message envelope wrapping user messages, requests, and system signals.

`Envelope` is NOT Clone because `Request` contains a oneshot sender.
Use `Message` or `Signal` variants where cloning is needed.


### `struct Message`

An application-level message sent between actors.


### `struct Request`

A request expecting a response, carrying a oneshot reply channel.


### `enum MessagePayload`

Dynamic message payload (typed via serde-like serialization in practice).


### `enum Signal`

System signals for process lifecycle management.


### `enum ExitReason`

Reason a process exited.


### `fn new_mailbox`

Create a new mailbox with the given buffer capacity.


### `struct RequestContext`

Lightweight request context for tracing and provenance.


### `struct Pid`

Process identifier for actors in the Vox runtime.
Unique within a runtime instance.


## Module: `vox-runtime\src\populi.rs`

# Populi LLM Client

`PopuliClient` communicates with the Populi LLM (local or remote) for
code generation, text embedding, classification, and fine-tuning data submission.

## Modes
- **Local**: `http://localhost:11434` (Ollama-compatible API)
- **Remote**: `https://api.populi.dev` (configurable)


### `struct PopuliConfig`

Configuration for connecting to Populi.


### `struct GenerateResponse`

A generation response.


### `struct EmbedResponse`

An embedding response.


### `struct Classification`

Classification result.


### `struct PopuliClient`

The main Populi LLM client.


### `struct ProcessContext`

Internal state of a running actor process.


### `struct ProcessHandle`

External handle to a running actor process, used to send messages.


### `enum CallError`

Errors that can occur during a `call()` request.


### `fn spawn_process`

Spawn a new actor process with the given behavior function.
Returns a ProcessHandle for communication.


## Module: `vox-runtime\src\prompt_canonical.rs`

Prompt canonicalization pipeline to reduce LLM failure modes.

Normalizes structure, extracts objectives, detects conflicts, and produces
order-invariant representations so that model behavior is less sensitive
to the order in which the user states things.


### `struct CanonicalizedPrompt`

Result of canonicalization (normalized text + optional metadata for transparency).


### `struct Objective`

A single objective extracted from a prompt.


### `struct Conflict`

A detected conflict between two instructions.


### `fn canonicalize`

Canonicalize a raw prompt: normalize whitespace, section boundaries, and structure.
Produces a stable representation for hashing and downstream use.


### `fn payload_hash`

Extract a short hash of the input for debug logging (e.g. payload sent to parser).


### `fn extract_objectives`

Extract objective-like sentences or bullets from a prompt (heuristic).


### `fn detect_conflicts`

Detect likely conflicting instructions (simple keyword/negation heuristics).


### `fn order_invariant_pack`

Build an order-invariant packed prompt: objectives as a numbered list so order is explicit.


### `fn safety_pass`

Safety pass: reject or sanitize prompts that look like injection attempts.
Returns Ok(sanitized) or Err(SafetyError) if rejected.


### `fn canonicalize_prompt`

Full pipeline: canonicalize, extract objectives, detect conflicts, optionally pack.
Use this at task ingress or before LLM generate for maximum consistency.


### `fn canonicalize_simple`

Convenience: canonicalize only (no safety pass, no order-invariant pack).
Use when you just want normalized whitespace and structure.


### `struct RateLimiter`

In-memory fixed-window rate limiter for generated services.


### `struct ProcessRegistry`

Global registry mapping Pids and names to live process handles.


### `struct RetryPolicy`

Retry policy for resilient outbound HTTP calls.


### `struct ResilientHttpClient`

HTTP client with retry and endpoint fallback support.


### `struct RetrievedChunk`

Retrieval chunk with provenance metadata.


### `struct ContextBudget`

Context budget to cap prompt growth.


### `struct ProvenanceRecord`

Compact provenance attachment for observability and reproducibility.


### `fn apply_context_budget`

Select top-scoring chunks and enforce max char budget.


### `struct Scheduler`

Cooperative scheduler for the Vox actor runtime.
Uses Tokio's work-stealing executor under the hood, with
reduction counting in each ProcessContext for fairness.


### `struct LocalStorage`

Basic local storage backend for storing and retrieving files.


### `struct StateStore`

A SQLite-backed KV store for Actor state.


## Module: `vox-runtime\src\subscription.rs`

Reactive subscription manager for table-level change notifications.

Uses `tokio::sync::broadcast` channels to notify subscribers when
a table's data has been mutated. This powers SSE-based reactive queries.

# Architecture

```text
@mutation insert_task()
│
▼
SubscriptionManager::notify("tasks")
│
▼
broadcast::Sender<()> ──► all Receivers for "tasks"
│
▼
SSE endpoint re-runs @query list_tasks()
│
▼
Client gets updated result
```


### `struct SubscriptionManager`

Manages per-table broadcast channels for reactive query subscriptions.

When a `@mutation` commits, it calls `notify()` with the affected table names.
SSE subscription endpoints hold `Receiver` handles and re-run their queries
when notified.


### `enum RestartStrategy`

Restart strategy for supervised processes.


### `struct ChildSpec`

Specification for a supervised child process.


### `struct Supervisor`

A supervisor managing a set of child actor processes.


## Module: `vox-runtime\src\transport.rs`

Stream transport for chat and real-time endpoints.

Generated Vox apps use **SSE (Server-Sent Events)** by default for streaming
chat and subscription updates. **WebSocket** is reserved for future use when
high-frequency bidirectional streams are needed (e.g. low-latency token streams).
The runtime and codegen use this enum so a future WebSocket path can be added
without breaking the API.


### `enum StreamTransport`

Identifies the transport used for a streaming endpoint.


## Module: `vox-runtime\src\builtins.rs`

Native hashing and identity primitives. Compiled Vox programs call these through
the `std.*` syntax; the codegen emits direct Rust function calls with no intermediary.

See [Builtins: Hashing & Identity](../ref-builtins-hashing.md) for the full reference including
benchmark estimates, collision avoidance design, and when to use each tier.


### `fn vox_hash_fast`

Fast non-cryptographic hash using **XXH3-128** (twox-hash crate).

```rust
pub fn vox_hash_fast(input: &str) -> String
```

- Output: 32-character lowercase hex (128-bit)
- Rate: ~60 GB/s on 4 KB inputs
- Deterministic across machines
- **Not cryptographic** — use only where inputs are controlled

Vox syntax: `std.hash_fast(x)` or `std.crypto.hash_fast(x)`


### `fn vox_hash_secure`

Cryptographic hash using **BLAKE3-256** (blake3 crate).

```rust
pub fn vox_hash_secure(input: &str) -> String
```

- Output: 64-character lowercase hex (256-bit)
- Rate: ~14 GB/s on 4 KB inputs (5–7× faster than SHA-256)
- Collision resistance ≈ 2⁻¹²⁸
- Safe for permanent storage, provenance, and cross-machine deduplication

Vox syntax: `std.crypto.hash_secure(x)`


### `fn vox_uuid`

Monotonic unique identifier combining nanosecond timestamp and atomic counter.

```rust
pub fn vox_uuid() -> String
```

- Format: `vox-{16-hex-nanos}-{16-hex-counter}`
- Guaranteed unique within a process even at sub-nanosecond resolution
- Rate: > 10 million/second

Vox syntax: `std.uuid()` or `std.crypto.uuid()`


### `fn vox_now_ms`

Current UNIX time in milliseconds.

```rust
pub fn vox_now_ms() -> u64
```

Vox syntax: `std.now_ms()` or `std.time.now_ms()`
