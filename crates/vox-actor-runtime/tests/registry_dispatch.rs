//! Phase 5.2: ActorRegistry register + dispatch.

use serde_json::json;
use std::sync::Arc;
use vox_actor_runtime::registry::ActorRegistry;

#[tokio::test]
async fn registered_actor_dispatches_to_handler() {
    let registry = ActorRegistry::new();
    registry
        .register(
            "Counter",
            Arc::new(|args: serde_json::Value| {
                Box::pin(async move {
                    let n: i64 = args.get("n").and_then(|v| v.as_i64()).unwrap_or(0);
                    Ok(json!(n + 1))
                })
            }),
        )
        .await;

    let result = registry
        .dispatch("Counter", "inc", json!({ "n": 5 }))
        .await
        .expect("dispatch");
    assert_eq!(result, json!(6));
}

#[tokio::test]
async fn dispatch_unknown_actor_errors() {
    let registry = ActorRegistry::new();
    let result = registry.dispatch("DoesNotExist", "ping", json!({})).await;
    assert!(result.is_err(), "dispatch to unknown actor must error");
}

#[tokio::test]
async fn registry_is_cloneable_and_shared() {
    let registry = ActorRegistry::new();
    registry
        .register("Echo", Arc::new(|args| Box::pin(async move { Ok(args) })))
        .await;
    let clone = registry.clone();
    let result = clone
        .dispatch("Echo", "send", json!({"hello": "world"}))
        .await
        .unwrap();
    assert_eq!(result, json!({"hello": "world"}));
}
