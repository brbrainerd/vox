//! D3 end-to-end: an emitted actor's mailbox loop decodes the `{"event","args"}`
//! wire format, calls the matching handler, mutates state (Message path), and
//! replies (Request path). Before this, the generated loop dropped every
//! message (`let _ = envelope; // dispatch not yet wired`).
//!
//! This test hand-mirrors the exact structure `emit_actor_body` generates (see
//! `crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs` and the spec
//! `docs/superpowers/specs/2026-06-03-actor-message-wire-format-design.md`).
//! The companion codegen test `actor_dispatch_table_routes_to_handlers`
//! (durability_lowering.rs) asserts the emitter actually produces this shape.

#![allow(non_snake_case)]
// This test deliberately hand-mirrors `emit_actor_body` codegen output (see module docs),
// so keep the emitted shape rather than clippy's stylistic rewrites.
#![allow(clippy::single_match, clippy::let_unit_value)]

use vox_actor_runtime::{Envelope, Message, MessagePayload, Pid, ProcessContext, spawn_process};

#[derive(Default)]
struct CounterState {
    total: i64,
}

// Stand-ins for the emitted `Counter_<event>(state, …)` handler functions.
fn Counter_add(state: &mut CounterState, n: i64) {
    state.total += n;
}
fn Counter_get(state: &mut CounterState) -> i64 {
    state.total
}

#[tokio::test]
async fn emitted_actor_dispatch_delivers_mutates_and_replies() {
    // ── This block is the literal shape of `emit_actor_body`'s output. ──
    let handle = spawn_process(move |mut ctx| async move {
        fn __vox_arg<__T: ::serde::de::DeserializeOwned>(
            args: &::serde_json::Value,
            i: usize,
        ) -> ::std::option::Option<__T> {
            args.get(i)
                .and_then(|__a| ::serde_json::from_value(__a.clone()).ok())
        }
        #[allow(unused_mut, unused_variables)]
        let mut _state = CounterState::default();
        while let Some(envelope) = ctx.receive().await {
            match envelope {
                Envelope::Message(__m) => {
                    let __v: ::serde_json::Value = match __m.payload.deserialize_json() {
                        Ok(__v) => __v,
                        Err(_) => continue,
                    };
                    let __ev = __v.get("event").and_then(|__e| __e.as_str()).unwrap_or("");
                    let __args = __v
                        .get("args")
                        .cloned()
                        .unwrap_or(::serde_json::Value::Null);
                    match __ev {
                        "add" => {
                            let n: i64 = match __vox_arg(&__args, 0usize) {
                                Some(__a) => __a,
                                None => continue,
                            };
                            let _ = Counter_add(&mut _state, n);
                        }
                        _ => {}
                    }
                }
                Envelope::Request(__req) => {
                    let __v: ::serde_json::Value = match __req.payload.deserialize_json() {
                        Ok(__v) => __v,
                        Err(_) => continue,
                    };
                    let __ev = __v
                        .get("event")
                        .and_then(|__e| __e.as_str())
                        .unwrap_or("")
                        .to_string();
                    let __args = __v
                        .get("args")
                        .cloned()
                        .unwrap_or(::serde_json::Value::Null);
                    match __ev.as_str() {
                        "get" => {
                            let __ret = Counter_get(&mut _state);
                            ProcessContext::reply(
                                __req,
                                ::serde_json::to_vec(&__ret).unwrap_or_default(),
                            );
                        }
                        _ => {}
                    }
                }
                Envelope::Signal(_) => {}
            }
        }
    });

    // Fire-and-forget Messages mutate state (proves the Message decode path).
    for n in [5_i64, 3, 4] {
        let payload =
            MessagePayload::json_value(&serde_json::json!({ "event": "add", "args": [n] }));
        handle
            .send(Envelope::Message(Message {
                from: Pid::new(),
                payload,
            }))
            .await
            .expect("send add");
    }

    // A Request reads the accumulated state back (proves Request decode + reply).
    // The FIFO mailbox guarantees the three adds are processed before `get`.
    let reply = handle
        .call(MessagePayload::json_value(
            &serde_json::json!({ "event": "get", "args": [] }),
        ))
        .await
        .expect("call get");
    let total: i64 = serde_json::from_slice(&reply).expect("decode reply");
    assert_eq!(
        total, 12,
        "5 + 3 + 4 delivered to the handler and summed in state"
    );

    // An unknown event is dropped without panicking; the loop survives.
    let unknown = MessagePayload::json_value(&serde_json::json!({ "event": "nope", "args": [] }));
    handle
        .send(Envelope::Message(Message {
            from: Pid::new(),
            payload: unknown,
        }))
        .await
        .expect("send unknown");
    let reply2 = handle
        .call(MessagePayload::json_value(
            &serde_json::json!({ "event": "get", "args": [] }),
        ))
        .await
        .expect("call get after unknown");
    let total2: i64 = serde_json::from_slice(&reply2).expect("decode reply2");
    assert_eq!(total2, 12, "unknown event dropped, state and loop intact");
}
