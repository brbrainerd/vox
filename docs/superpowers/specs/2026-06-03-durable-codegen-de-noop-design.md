# De-no-op emitted durable codegen (workflows / activities / actors) — Design

**Date:** 2026-06-03
**Status:** Proposed, awaiting approval before implementation.
**Track:** D (golden-corpus & compiler-reality plan).
**Context:** The interpreter durable path is real (VoxDbTracker crash-replay is tested), but the **emitted Rust binary** is hollow for the language's flagship durability story. Reference: [ADR-019](../../src/adr/019-durable-workflow-journal-contract-v1.md), [ADR-021](../../src/adr/021-generated-workflow-durability-parity.md), [ADR-041](../../src/adr/041-durable-functions-completion-2026.md), [durability-runtime-audit-2026.md](../../src/architecture/durability-runtime-audit-2026.md).

## Verified current state (read 2026-06-03, `crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs`)

| Construct | Emitter | Reality | Verified |
|---|---|---|---|
| **Actor** | `emit_actor_body` (:80-123) | Spawns the mailbox loop but **drops every message**: `while let Some(envelope) = ctx.receive().await { let _ = envelope; // dispatch not yet wired }` (:118-120). Comment (:101-107) states the no-op is intentional pending the `Envelope` wire-shape decision. | ✅ confirmed |
| **Workflow** | `emit_workflow_body` (:31-56) | Lowers to `interpret_workflow_durable(hir, name, &mut DefaultTracker)` (:42-46). Uses **`DefaultTracker`**, not a persisting `VoxDbTracker` — so the emitted workflow interprets but does not crash-durably journal. | ✅ confirmed it uses DefaultTracker; **VERIFY** DefaultTracker is non-persisting |
| **Activity** | `emit_activity_body` (:58-78) | Wraps the body in `vox_workflow_runtime::journal::execute(activity_id, async move {…})` (:70). | ⚠️ scout claims `journal::execute` records only under `cfg(test)` — **VERIFY `journal/execute.rs:15-46` first** |

## Goal

Bring the emitted binary to parity with the interpreter's durable subset (the **linear** subset — the planner already bails on arbitrary `match`/loops; keep that boundary):
1. Actors deliver messages to their handlers.
2. Workflows journal steps durably (survive restart) via a real tracker.
3. Activities journal results + retries in production, not only under test.

## Design

### D3 — Actor `Envelope` dispatch (clearest, do first)

The blocker is a wire-shape decision, now made here: `vox_actor_runtime::Envelope` is the tagged enum `Message(Message) | Request(Request) | Signal(Signal)`. The emitted mailbox loop must `match` the envelope and route the inner `MessagePayload` to the handler whose `on <event>(...)` it names.

```rust
// emitted into the actor shell loop:
while let Some(envelope) = ctx.receive().await {
    match envelope {
        ::vox_actor_runtime::Envelope::Message(m) => {
            match m.payload_tag() /* or match on the payload enum */ {
                "join"  => { /* call ChatRoom::join(state, decoded_args) */ }
                "leave" => { /* ... */ }
                _ => { /* unknown message — log + drop */ }
            }
        }
        ::vox_actor_runtime::Envelope::Request(_) => { /* req/reply path */ }
        ::vox_actor_runtime::Envelope::Signal(_)  => { /* lifecycle */ }
    }
}
```

The handler bodies already emit correctly (`emit_actor_body` returns `emit_plain_body` for `Name::event` fns, :94-96). What's missing is the **dispatch table** that decodes the payload and calls them. `emit_actor_body` has the `actor_handlers: &[&HirFn]` list (currently `_handlers`, :84) — use it to generate the match arms (one per `Name::event` handler), decoding args from the message payload.

**Open decision to confirm before coding:** how a `MessagePayload` carries the event name + typed args (the runtime's message-encoding contract). Read `vox_actor_runtime`'s `Envelope`/`Message`/`MessagePayload` definitions and mirror the interp's actor dispatch (which works) for the encoding.

### D1 — Workflow real tracker

Replace `DefaultTracker` (:42) with a `VoxDbTracker` (the real persisting tracker the interpreter uses for crash-replay). The emitted body becomes:

```rust
let mut __vox_tracker = ::vox_workflow_runtime::workflow::tracker::VoxDbTracker::open(/* db handle from ctx */)?;
let __vox_journal = ::vox_workflow_runtime::workflow::interpret_workflow_durable(__vox_hir, "name", &mut __vox_tracker).await?;
```

Confirm: (a) `VoxDbTracker` exists and is constructible from the generated server's db handle; (b) `DefaultTracker` is indeed non-persisting (so this is a real fix, not cosmetic). Train only the linear subset.

### D2 — Activity journal in production

Verify `journal::execute` (`journal/execute.rs:15-46`) persists outside `cfg(test)`. If it is test-only, give it a production path that records the activity result + drives retries via the same tracker as D1.

### D4/D5/D6 — corpus honesty (already partially done; cheap, no codegen)

- `saga_compensation.vox`: no saga runtime exists (grep=0) — reclassify as honest manual compensation, not a durable saga.
- `scheduled_tick.vox`: the scheduler IS shipped (`runner.rs` + `main_boot.rs`); rewrite to real `@scheduled` and drop the false E028 claim (E028 is retired).
- Determinism lint: broaden the 5-path blocklist; add a negative golden.

## Implementation order (TDD)

1. **D3 actor dispatch** (highest value, self-contained): integration test that an emitted actor delivers a message to its handler (assert a state mutation / reply). Settle the payload-encoding contract first.
2. **D1 workflow tracker**: integration test that an emitted workflow journals a step (assert journal rows persist + replay).
3. **D2 activity**: test emitted activity records a result + retries.
4. **D4/D5/D6** corpus + lint (no codegen; can be done independently and in parallel).

## Risks

- **Encoding contract.** D3 hinges on the message payload → handler-args decoding contract; get it from the runtime + interp, do not invent it.
- **DB handle plumbing.** D1/D2 need the generated server's db handle reachable from the workflow/activity body — confirm the generated server wires it.
- **Scope creep.** Keep to the linear subset the planner already supports (`plan.rs:437` bails on `match` — respect that boundary; do not try to make arbitrary control flow durable).

## Done when

- An emitted actor routes messages to handlers; an emitted workflow persists + replays its journal; an emitted activity records results in production.
- `saga_compensation.vox`/`scheduled_tick.vox` are honest; determinism lint has a negative golden.
- Generated-binary integration tests cover each; existing CR-P1 health checks still pass.
