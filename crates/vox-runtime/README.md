# vox-runtime

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

Content-addressable file storage with FNV-1a hashing:

- Deterministic IDs for identical content
- Automatic deduplication
- URL-based access via `/storage/{id}` prefix

## Key Design Decisions

1. **No `null`**: All optional values use `Option<T>`, following Vox's zero-null philosophy
2. **Lock safety**: All `RwLock`/`Mutex` operations use `.expect()` with descriptive messages instead of `.unwrap()`
3. **Content addressing**: Storage uses content hashing (FNV-1a) instead of timestamp-based IDs for deterministic behavior
