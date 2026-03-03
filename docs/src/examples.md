# Examples

All examples are located in the [`examples/`](../../examples/) directory. Each demonstrates a key feature of the Vox language.

---

## Quick Reference

| Example | Features Demonstrated |
|---------|----------------------|
| [`simple_server_fn.vox`](#server-functions) | `@server`, `@component`, auto-generated fetch |
| [`chatbot.vox`](#full-stack-chatbot) | `@component`, `@table`, `@query`, `@mutation`, `@action`, actors, `style`, `routes` |
| [`chatbot_server_fn.vox`](#chatbot-with-server-functions) | `@server`, `@component`, ADT pattern matching in JSX |
| [`data_layer.vox`](#data-layer) | `@table`, `@index`, `@server`, typed database API |
| [`actor.vox`](#actors) | `actor`, `state`, `spawn`, `send`, `message` |
| [`durable_counter.vox`](#durable-counter) | Actor with `state_load`/`state_save`, `@component`, `routes` |
| [`workflow.vox`](#workflows) | `workflow`, `activity`, `with` expression |
| [`durable_execution.vox`](#durable-execution) | Activities with retry/timeout/backoff policies |
| [`mcp_tool.vox`](#mcp-tools) | `@mcp.tool`, `@mcp.resource` |
| [`agent.vox`](#ai-agents) | `@agent_def`, `@mcp.tool`, agent memory |
| [`dashboard.vox`](#dashboard) | `@v0`, `@component`, `http`, `routes` |
| [`sharing.vox`](#sharing--skills) | `@skill`, `workflow`, `@component`, publishable packages |
| [`testing.vox`](#testing) | `@test`, `assert`, ADT matching |
| [`server_fn.vox`](#minimal-server-function) | Minimal `@server` example |

---

## Server Functions

**File**: `examples/simple_server_fn.vox`

Demonstrates how `@server` generates both a backend HTTP route and a frontend fetch wrapper:

```vox
# Skip-Test
@server fn greet(name: str) to Greeting:
    ret Hello("Welcome, " + name + "!")

@component fn App() to Element:
    let result = use_state("")
    let handle_click = fn(_e):
        let greeting = greet("Alice")     # ← auto-generated fetch call
        set_result(greeting)
    ret <div><button onClick={handle_click}>Greet</button><p>{result}</p></div>
```

**Key insight**: `greet("Alice")` in the component compiles to a typed `fetch()` call — the compiler generates the HTTP client automatically.

---

## Full-Stack Chatbot

**File**: `examples/chatbot.vox`

A complete application in a single file: React UI + Axum backend + database + actors:

```vox
# Skip-Test
@component fn Chat() to Element:
    let (messages, set_messages) = use_state([])
    let (input, set_input) = use_state("")
    let send = fn (_e) (set_messages(messages.append({role: "user", text: input})),
                        spawn(ChatClient).send(input), set_input(""))
    <div class="chat_container">
        <div class="messages">
            for msg in messages:
                <div class="msg {msg.role}">{msg.text}</div>
        </div>
        <div class="input_area">
            <input class="chat_input" bind={input} placeholder="Type a message..." />
            <button class="send_btn" on_click={send}>Send</button>
        </div>
    </div>
```

This single file defines: UI components, styling, routes, database tables, queries, mutations, actions, and actors.

---

## Chatbot with Server Functions

**File**: `examples/chatbot_server_fn.vox`

Shows ADT pattern matching inside JSX for rendering different message types:

```vox
# Skip-Test
type Message =
    | User(text: str)
    | Assistant(text: str)

{messages.map(fn(msg) match msg:
    | User(text) -> <div className="user-message">{text}</div>
    | Assistant(text) -> <div className="assistant-message">{text}</div>
)}
```

---

## Data Layer

**File**: `examples/data_layer.vox`

Typed database tables with indexes and server functions:

```vox
# Skip-Test
@table type Task:
    title: str
    done: bool
    priority: int
    owner: str

@index Task.by_done on (done, priority)
@index Task.by_owner on (owner)

@server fn list_tasks() to List[Task]:
    ret db.query(Task).collect()

@server fn add_task(title: str, owner: str) to Id[Task]:
    ret db.insert(Task, { title: title, done: false, priority: 0, owner: owner })
```

---

## Actors

**File**: `examples/actor.vox`

The actor model with state, message handlers, and inter-actor communication:

```vox
# Skip-Test
actor Counter:
    state count: int = 0
    on increment(amount: int) to int:
        count = count + amount
        count
    on get_count() to int:
        count

fn main():
    let counter = spawn(Counter)
    let new_count = counter.send(increment(5))   # returns 5
    let _ = counter.send(increment(3))
    let total = counter.send(get_count())         # returns 8
```

---

## Durable Counter

**File**: `examples/durable_counter.vox`

An actor whose state persists across restarts via `state_load`/`state_save`:

```vox
# Skip-Test
actor PersistentCounter:
    on increment() to int:
        let current = state_load("counter")
        let next = current + 1
        state_save("counter", next)
        ret next
```

---

## Workflows

**File**: `examples/workflow.vox`

Durable workflows with activities and the `with` expression:

```vox
# Skip-Test
activity fetch_user_data(user_id: str) to Result[str]:
    ret Ok("User data for " + user_id)

workflow onboard_user(user_id: str, email: str) to Result[str]:
    let profile = fetch_user_data(user_id) with { retries: 3, timeout: "30s" }
    let _ = send_notification(email, "Welcome! " + profile) with { retries: 5, timeout: "60s" }
    ret Ok("Onboarding complete for " + user_id)
```

---

## Durable Execution

**File**: `examples/durable_execution.vox`

Full workflow with varied retry policies:

```vox
# Skip-Test
workflow process_order(customer: str, order_data: str, amount: int) to Result[str]:
    let validated = validate_order(order_data) with { timeout: "5s" }
    let payment = charge_payment(amount, "card-123") with { retries: 3, timeout: "30s", initial_backoff: "500ms" }
    let confirmation = send_confirmation(customer, "order-001") with { retries: 2, activity_id: "confirm-order-001" }
    ret confirmation
```

---

## MCP Tools

**File**: `examples/mcp_tool.vox`

Expose functions as AI-discoverable tools via the Model Context Protocol:

```vox
# Skip-Test
@mcp.tool("create_note", "Create a new note with a title and content")
fn create_note(title: str, content: str) to str:
    ret "Created note: " + title

@mcp.resource("notes://recent", "List of recently created notes")
fn recent_notes() to list[str]:
    ret ["Recent note 1", "Recent note 2"]
```

---

## AI Agents

**File**: `examples/agent.vox`

Define AI agents with memory and tool access:

```vox
# Skip-Test
@agent_def fn SupportBot(query: str, session: str) to str:
    let past = db.agent_memory.find(session)
    let response = "Based on " + past.context + " -> " + query
    db.agent_memory.insert(AgentMemory(session, query))
    ret response
```

---

## Dashboard

**File**: `examples/dashboard.vox`

AI-generated UI + hand-coded components + routing:

```vox
# Skip-Test
@v0 "A metrics dashboard with cards showing KPIs and a line chart" fn Dashboard() to Element

@component fn ChatWidget() to Element:
    # ... hand-coded component

routes:
    "/" to Dashboard
    "/chat" to ChatWidget
```

---

## Sharing & Skills

**File**: `examples/sharing.vox`

Publishable skills and reusable components:

```vox
# Skip-Test
@skill fn DataSummarizer(text: str) to str:
    "Summary of " + text

workflow process_document(doc_id: str) to Result[bool]:
    let doc = db.documents.find(doc_id)
    let summary = DataSummarizer(doc.content)
    db.documents.update(doc_id, summary)
    ret Ok(true)
```

---

## Testing

**File**: `examples/testing.vox`

Unit tests with `@test` and `assert`:

```vox
# Skip-Test
@test fn test_addition() to Unit:
    let sum = 1 + 2
    assert(sum is 3)

@test fn test_str_cast() to Unit:
    let n = 42
    let s = str(n)
    assert(s is "42")
```

---

## Minimal Server Function

**File**: `examples/server_fn.vox`

The simplest possible server function:

```vox
# Skip-Test
type Greeting =
    | Hello(message: str)

@server fn greet(name: str) to Greeting:
    ret Hello("Welcome, " + name + "!")
```
