# Example: Vox Chatbot Demo

```vox
# Vox Chatbot Demo
# ─────────────────────────────────────────────────────
# A full-stack AI chatbot that demonstrates many Vox language features:
#   • Type definitions (tagged unions / ADTs)
#   • @component decorator for React-like UI components
#   • JSX syntax for declarative rendering
#   • style: blocks for scoped CSS-in-Vox
#   • routes: block for client-side routing
#   • @table, @query, @mutation, @action decorators for data layer
#   • actor declarations for message-passing concurrency
#   • Reactive bindings with bind={} and event handlers

# ── Imports ──────────────────────────────────────────
# `import` pulls in external dependencies.
# Here we import React's useState hook for local component state.
import react . use_state

# ── Type Definitions ─────────────────────────────────
# `type` defines a tagged union (Algebraic Data Type / ADT).
# Each variant can carry typed fields.
# The Vox compiler generates matching Rust enums and TypeScript unions.
type ChatResult =
    | Success(text: str)
    | Error(msg: str)

# ── Components ───────────────────────────────────────
# `@component` marks a function as a reactive UI component.
# Components return `Element` (JSX). The codegen produces
# a React function component in TypeScript.
@component fn Chat() to Element:
    # `use_state` returns a (value, setter) tuple.
    # Vox destructures them with `let (a, b) = ...` syntax.
    let (messages, set_messages) = use_state([])
    let (input, set_input) = use_state("")

    # Lambda / closure: `fn (params) body`
    # `spawn(Actor)` creates a new actor instance and `.send()` dispatches a message.
    let send = fn (_e) (set_messages(messages . append({role: "user", text: input})), spawn (ChatClient) . send(input), set_input(""))

    # JSX syntax: `< tag attr=value > children </ tag >`
    # Vox JSX uses spaces around angle brackets for readability.
    < div class = "chat_container" >
        < div class = "messages" >
            # `for ... in ...` loops render list items
            for msg in messages:
                < div class = "msg {msg.role}" >
                    # Curly braces `{expr}` interpolate expressions inside JSX
                    {msg . text}
                </ div >
        </ div >
        < div class = "input_area" >
            # `bind={var}` creates a two-way binding (sugar for value + onChange)
            # `on_key_up` attaches an event handler
            < input class = "chat_input" bind = {input} on_key_up = {fn (e) e . key is "Enter" and send(e)} placeholder = "Type a message..." />
            < button class = "send_btn" on_click = {send} > Send </ button >
        </ div >
    </ div >

# ── Scoped Styles ────────────────────────────────────
# `style:` blocks define CSS scoped to the component.
# Properties use camelCase (converted to kebab-case in output).
style:
    . chat_container:
        fontFamily: "sans-serif"
        display: "flex"
        flexDirection: "column"
        height: "100vh"
        maxWidth: "800px"
        margin: "0 auto"
        borderLeft: "1px solid #eee"
        borderRight: "1px solid #eee"

    . messages:
        flex: "1"
        overflowY: "auto"
        padding: "24px"
        display: "flex"
        flexDirection: "column"
        gap: "16px"

    . msg:
        padding: "12px 16px"
        borderRadius: "12px"
        maxWidth: "80%"
        lineHeight: "1.5"

    . user:
        backgroundColor: "#007AFF"
        color: "white"
        alignSelf: "flex-end"
        borderBottomRightRadius: "4px"

    . bot:
        backgroundColor: "#f0f0f0"
        color: "#333"
        alignSelf: "flex-start"
        borderBottomLeftRadius: "4px"

    . input_area:
        padding: "24px"
        borderTop: "1px solid #eee"
        display: "flex"
        gap: "12px"
        background: "white"

    . chat_input:
        flex: "1"
        padding: "12px"
        border: "1px solid #ddd"
        borderRadius: "8px"
        fontSize: "16px"
        outline: "none"

    . send_btn:
        backgroundColor: "#007AFF"
        color: "white"
        border: "none"
        padding: "0 24px"
        borderRadius: "8px"
        fontWeight: "600"
        cursor: "pointer"
        transition: "background 0.2s"

# ── Routes ───────────────────────────────────────────
# `routes:` maps URL paths to component names.
# The codegen generates a React Router (TS) or Axum router (Rust).
routes:
    "/" to Chat

# ── Data Layer ───────────────────────────────────────
# `@table` defines a database table with typed fields.
# The codegen generates CREATE TABLE DDL and Rust/TS structs.
@table type MessageRecord:
    role: str
    text: str

# `@query` marks a read-only database function.
# Must have an explicit return type; codegen generates
# a GET endpoint with caching semantics.
@query fn get_history() to list[MessageRecord]:
    # The runtime injects the database connection automatically
    ret []

# `@mutation` marks a write function.
# Codegen generates a POST endpoint with transaction semantics.
@mutation fn save_message(role: str, text: str) to Result[bool]:
    # Store message to the MessageRecord table
    ret Ok(true)

# `@action` runs server-side logic that can call queries/mutations.
# Actions can spawn actors and invoke other side-effectful code.
@action fn handle_chat(prompt: str) to Result[str]:
    let _ = save_message("user", prompt)
    # `spawn(Actor)` creates an actor instance; `.send(msg)` dispatches
    let openai = spawn (OpenRouterActor)
    let response = openai . send(prompt)
    let _ = save_message("bot", response)
    ret Ok(response)

# ── Actors ───────────────────────────────────────────
# `actor` declares a concurrent message-passing unit.
# Each actor runs in its own Tokio task with a bounded mailbox.
# `on <method>(params) to <ReturnType>:` defines message handlers.
actor OpenRouterActor:
    on send(msg: str) to str:
        # In production, this would call the OpenRouter API
        "Hello from Vox! You said: " + msg

# Client-side actor for dispatching chat actions
actor ChatClient:
    on send(msg: str) to Unit:
        # Invokes the server-side @action
        let _ = msg
```
