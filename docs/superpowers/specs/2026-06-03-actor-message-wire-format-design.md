# Actor message wire format — Design

**Date:** 2026-06-03
**Status:** Proposed, awaiting approval before implementation.
**Track:** D3 (golden-corpus & compiler-reality plan — unblocks the actor de-no-op in [`2026-06-03-durable-codegen-de-noop-design.md`](2026-06-03-durable-codegen-de-noop-design.md) §D3).
**Context:** Emitted actors spawn a mailbox loop but **drop every message**. The blocker, stated in the D3 spec §D3 "Open decision", is the wire-shape contract: how a `MessagePayload` carries the event name + typed args so the dispatch table can decode and call `Actor::<event>(state, args)`. This spec settles that contract for both the **send side** and the **dispatch side**.

## Problem

The generated actor mailbox loop is a no-op. There is no defined encoding for "invoke handler `on greet(name)` with `name = "ada"`" as bytes on the wire, so the emitted dispatch table cannot exist. Until a concrete `MessagePayload` shape is chosen and justified against the runtime types, Track D3 cannot be implemented.

## Verified current state (read 2026-06-03)

### Runtime message types — `crates/vox-actor-runtime/src/mailbox.rs`

| Item | Location | Reality |
|---|---|---|
| `MessagePayload` | `mailbox.rs:33-40` | Tagged enum `Text(Bytes) \| Json(Bytes) \| Binary(Bytes)`. All `Bytes`-backed (zero-copy clone). |
| Constructors | `mailbox.rs:44-61` | `text(s)`, `json_value(&serde_json::Value)`, `json_str(s)`, `binary(b)`. |
| Decoders | `mailbox.rs:71-78` | `as_str() -> Option<&str>`, `deserialize_json::<T>() -> serde_json::Result<T>`. |
| `Envelope` | `mailbox.rs:89-97` | `Message(Message) \| Request(Request) \| Signal(Signal)`. **Not `Clone`** (Request holds a oneshot). |
| `Message` | `mailbox.rs:100-106` | `{ from: Pid, payload: MessagePayload }` — fire-and-forget. |
| `Request` | `mailbox.rs:110-117` | `{ from: Pid, payload: MessagePayload, reply_tx: oneshot::Sender<Bytes> }` — request/reply. |

There is **no** `event`/`tag`/`method` field on `Message` — the variant name discriminates only text vs json vs binary, not which handler to call. The event name must therefore live **inside** the payload bytes.

### Send / dispatch paths — `crates/vox-actor-runtime/src/process.rs`

| Item | Location | Reality |
|---|---|---|
| `ProcessHandle::send` | `process.rs:71-76` | `async send(&self, envelope: Envelope) -> Result<(), SendError<Envelope>>` — caller builds the whole `Envelope`. |
| `ProcessHandle::call` | `process.rs:80-92` | `async call(&self, payload: MessagePayload) -> Result<Bytes, CallError>` — wraps payload in `Envelope::Request` with a fresh `Pid::new()` sender, awaits reply bytes. |
| `ProcessContext::receive` | `process.rs:40-50` | `async receive(&mut self) -> Option<Envelope>` — the loop driver. |
| `ProcessContext::reply` | `process.rs:53-55` | `reply(request: Request, response: impl Into<Bytes>)` — sends reply bytes back through the oneshot. |
| `spawn_process` | `process.rs:113-128` | Takes a `FnOnce(ProcessContext) -> Future<Output=()>`, returns `ProcessHandle`. |

### Codegen — `crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs`

- `emit_actor_body` (`durability_lower.rs:80-123`). For a handler fn (`name.contains("::")`) it delegates to `emit_plain_body` (`:94-96`) — **handler bodies already emit correctly**. For the actor shell it emits the spawn loop with the no-op `let _ = envelope; // dispatch not yet wired` (`:118-120`).
- The handler list arrives as `_handlers: &[&HirFn]` (`:84`) — **currently unused**; this is the input to the dispatch table.
- The shell instantiates `<ActorName>State::default()` (`:114`); the state struct is emitted by `emit_actor_state_structs` (`workflow.rs:213-251`).

### How handlers + params lower — `crates/vox-compiler/src/hir/lower/decl.rs`

- `lower_actor_handlers` (`decl.rs:658-704`): each `on <event>(params)` becomes a standalone `HirFn` named `"ActorName::event_name"` (`:665`), carrying `params` (`:668`) and `return_type` (`:676`).
- The emitted Rust handler signature (`workflow.rs:177-192`) is `fn ActorName_event(state: &mut ActorNameState, <p>: <T>, …) -> <Ret>` — note `::` → `_` (`:175`), and the injected `state: &mut <Actor>State` first param (`:178-179`).
- AST source: `ActorHandler { event_name, params, return_type, body, … }` (`ast/decl/logic.rs:18-25`).

### No send-side surface today (verified)

Grep for actor message-send surface (`spawn(`, `.send(`, `ActorRef`, `send_message`) across `crates/vox-codegen` and `crates/vox-compiler/src/hir/lower` returns **no actor-send construct** — only the runtime's own `ProcessHandle::send`/`call`. The interpreter has **no actor dispatch at all** (no `Envelope` handling in the interp). Therefore the send side must be **defined**, not mirrored: there is no existing encoding to copy. The only existing producer of payloads is `ProcessHandle::call`, which takes a caller-supplied `MessagePayload` verbatim.

## Design decision: the wire format

**Choose `MessagePayload::Json` carrying a canonical envelope object:**

```json
{ "event": "<handler_event_name>", "args": [ <arg0>, <arg1>, … ] }
```

- `event`: the bare event name (the part after `::` in the handler's HIR name — e.g. `"greet"` for handler `MyActor::greet`).
- `args`: a JSON array, positionally aligned with the handler's `params` list (`decl.rs:668`). Each element is the `serde_json` serialization of the corresponding argument. Positional (not keyed-by-name) because handler params are an ordered `Vec` and codegen already iterates them positionally (`workflow.rs:181-192`); positional avoids depending on param-name stability and matches the call-site argument order.

### Why `Json`, not `Text` or `Binary`

1. **The runtime already has the exact decode primitive.** `MessagePayload::deserialize_json::<T>()` (`mailbox.rs:76-78`) and the `json_value` constructor (`mailbox.rs:49-51`) exist for precisely this. No new runtime API is required — the contract rides on already-shipped, already-tested methods (`payload_json_roundtrip` test, `mailbox.rs:196-201`).
2. **Self-describing event tag.** `Message` carries no event field (`mailbox.rs:100-106`), so the discriminator must be in-band. A JSON object with an `"event"` key is the minimal self-describing form; `Text` would force an ad-hoc delimiter parse and `Binary` would force a hand-rolled framing — both reinvent what `serde_json` gives free.
3. **Typed args for free.** Handler params have concrete HIR types (`int`, `str`, structs). `serde_json::Value` round-trips all of them and `deserialize_json` reconstructs the Rust type at the dispatch site. `Text` (one UTF-8 blob) cannot carry a typed multi-arg tuple without a second encoding layer.
4. **Zero-copy clone preserved.** `Json(Bytes)` clones O(1) (`mailbox.rs:32`, test `mailbox.rs:184-193`) — the deep-clone-across-mailbox concern (`DeepCloneToOwned`, `mailbox.rs:170-173`) is already satisfied by `Bytes`.

### Reply encoding (request/reply path)

For handlers with a `return_type`, the reply is the `serde_json` serialization of the return value, sent as raw `Bytes` through `ProcessContext::reply` (`process.rs:53-55`) — matching `ProcessHandle::call`'s `Result<Bytes, …>` (`process.rs:80`). The caller decodes those bytes with `serde_json::from_slice::<Ret>`. No envelope wrapper on the reply: `call` already pairs request→reply 1:1 via the oneshot, so the reply needs no `event` tag.

## Both sides of the contract

### (a) Send side — encoding a handler invocation

A send of `actor_handle.<event>(a0, a1)` encodes to:

```rust
let __vox_msg = ::serde_json::json!({
    "event": "<event>",
    "args": [ a0, a1 ],          // serde_json::json! serializes each in place
});
let __vox_payload = ::vox_actor_runtime::MessagePayload::json_value(&__vox_msg);
```

- **Fire-and-forget** (handler returns `Unit`/no `return_type`): wrap in `Envelope::Message(Message { from: Pid::new(), payload: __vox_payload })` and `handle.send(env).await` (`process.rs:71`).
- **Request/reply** (handler has a `return_type`): `let reply: Bytes = handle.call(__vox_payload).await?;` (`process.rs:80`) then `serde_json::from_slice::<Ret>(&reply)`.

**Surface-syntax note (out of scope for D3, recorded here for completeness).** There is no Vox send construct today (verified: grep = 0). D3 only needs the **dispatch side** to be real so emitted actors stop dropping messages; integration tests drive the send side by constructing the `MessagePayload` directly (as `ProcessHandle::call` already allows, `process.rs:80`). When a surface send is added later (`handle.greet("ada")` or `spawn`), it lowers to exactly the encoding above — this spec is the contract it must target. Until then the send side is a **runtime/codegen helper**, not new syntax.

### (b) Dispatch side — the emitted match table in `emit_actor_body`

`emit_actor_body` consumes `_handlers` (rename to `handlers`, `durability_lower.rs:84`) and generates one decode+call arm per `ActorName::event` handler. The no-op loop body (`:118-120`) is replaced by:

```rust
while let Some(envelope) = ctx.receive().await {
    match envelope {
        ::vox_actor_runtime::Envelope::Message(m) => {
            if let Some(__ev) = decode_event(&m.payload) {     // helper: read "event" str
                match __ev.as_str() {
                    "greet" => {
                        // decode positional args from "args":[…]
                        let __args = decode_args(&m.payload);
                        let name: String = from_arg(&__args, 0);
                        let _ = MyActor_greet(&mut _state, name);   // fire-and-forget: ignore Ret
                    }
                    // … one arm per handler …
                    _ => { /* unknown event — drop (optionally log) */ }
                }
            }
        }
        ::vox_actor_runtime::Envelope::Request(req) => {
            // same decode; for handlers WITH a return_type, run + reply:
            // let ret = MyActor_greet(&mut _state, name);
            // ::vox_actor_runtime::ProcessContext::reply(req, serde_json::to_vec(&ret).unwrap());
            let _ = req;
        }
        ::vox_actor_runtime::Envelope::Signal(_) => { /* lifecycle — drop for now */ }
    }
}
```

Codegen rules for generating each arm (all grounded in existing emit logic):
- **Arm key** = event name = `handler.name.split("::").nth(1)` (`decl.rs:665` guarantees the `Actor::event` shape).
- **Rust call name** = `handler.name.replace("::", "_")` (matches the emitted fn name, `workflow.rs:175`).
- **First call arg** = `&mut _state` (the shell binds `_state`, `durability_lower.rs:114`; matches the injected `state: &mut <Actor>State`, `workflow.rs:178-179`).
- **Remaining call args** = `handler.params` in order (`decl.rs:668`), each `serde_json`-decoded from `args[i]` into the param's emitted type (`emit_type(param.type_ann)`, reuse `workflow.rs:185-191`).
- **`Message` vs `Request` split:** handlers with no `return_type` are reachable from `Message` arms; handlers with a `return_type` are reachable from `Request` arms and must `ProcessContext::reply` (`process.rs:53`) with `serde_json::to_vec(&ret)`. Generate both match blocks from the same `handlers` list, partitioned by `handler.return_type.is_some()`.
- **`decode_event`/`decode_args`/`from_arg`** are emitted as small local helpers (or inlined): `m.payload.deserialize_json::<serde_json::Value>()` (`mailbox.rs:76`) then index `["event"]` / `["args"]`.

## Implementation steps (TDD sequence)

1. **Wire-format unit test (runtime).** In `mailbox.rs` tests, assert a round-trip: `json_value({"event":"greet","args":["ada"]})` → `deserialize_json::<Value>()` → `["event"] == "greet"`, `["args"][0] == "ada"`. (Locks the contract; uses only shipped APIs.) Red → green is trivial but pins the shape.
2. **Codegen dispatch-table test (red).** Extend `crates/vox-codegen/tests/durability_lowering.rs` (sibling of `actor_lowers_to_mailbox_spawn`, `:46-61`): emit `actor MyActor { on greet(name: str) to str { return name } }` and assert the emitted shell contains `Envelope::Message`, `match`, `"greet"`, and `MyActor_greet(&mut _state` — and that it **no longer** contains `// dispatch not yet wired`.
3. **Implement `emit_actor_body` dispatch (green).** Rename `_handlers` → `handlers` (`durability_lower.rs:84`); replace the no-op loop (`:118-120`) with the generated `match envelope { … }` per the rules above. Partition `handlers` by `return_type.is_some()` for the Message vs Request blocks. Reuse `emit_type` for arg decoding.
4. **Generated-binary integration test (the real proof).** Emit + compile a tiny actor crate; from a test, build a `MessagePayload::json_value({"event":"greet","args":["ada"]})`, send via `ProcessHandle::call`, assert the reply bytes deserialize to `"ada"` (proves Request decode + reply). Add a fire-and-forget actor that mutates state and assert the mutation (proves Message decode). This is the Track-D3 "actor delivers a message to its handler" gate.
5. **Negative/robustness arms.** Unknown event → dropped, loop continues (assert no panic). Malformed payload (`deserialize_json` errors) → dropped, loop continues.

## Test strategy

- **Runtime (`mailbox.rs` `#[cfg(test)]`):** the contract round-trip (step 1).
- **Codegen string-shape (`durability_lowering.rs`):** the emitted dispatch table contains the right arms and no longer contains the no-op marker (step 2); `plain_fn_unchanged` (`:63-86`) and `actor_lowers_to_mailbox_spawn` (`:46-61`) still pass (no regression to the spawn shell).
- **Generated-binary integration:** message delivery + reply round-trip on a real compiled actor (step 4) — the only test that proves the no-op is actually gone end-to-end.

## Risks

- **Param-name vs positional drift.** Using positional `args` means the dispatch arm must decode in the **same order** `lower_actor_handlers` stored params (`decl.rs:668`). Mitigation: the integration test (step 4) with a multi-arg handler exercises ordering; keep encode and decode both positional.
- **Type fidelity through JSON.** Non-JSON-native Vox types (if any reach a handler param) must serialize losslessly via `serde_json`. The emitted state/handler types already derive `Serialize/Deserialize` (`workflow.rs:92`, `:101`, `:232`), so this holds for the linear subset; flag any param type whose `emit_type` is not `Serialize`-able as out of scope.
- **Signal/lifecycle unhandled.** `Envelope::Signal` arms are drop-only for now (lifecycle is a separate track); document, don't silently imply support.
- **Reply for `Message`-sent handlers with a return type.** A handler with a `return_type` invoked via `Message` (fire-and-forget) discards the return value. That is correct (no oneshot to reply on); the value is only delivered on the `Request`/`call` path. Document so callers pick `call` when they need the result.

## Done when

- `MessagePayload::Json` with `{"event","args"}` is the documented contract, round-trip-tested in `mailbox.rs`.
- `emit_actor_body` uses `handlers` (no longer `_handlers`) to emit a `match envelope { Envelope::Message(m) => …, Envelope::Request(req) => …, Envelope::Signal(_) => … }` dispatch table; the `// dispatch not yet wired` marker is gone.
- A generated-binary integration test proves an emitted actor decodes a message, calls the handler, mutates state (Message path) and replies (Request path).
- `actor_lowers_to_mailbox_spawn` and `plain_fn_unchanged` still pass.
