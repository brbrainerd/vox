# Gamified Telemetry & Contributions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reward developer contributions and language usage by integrating gamified events for telemetry sharing, skill creation, peer sharing, and Vox compiler primitive milestones.

**Architecture:** We hook into trigger points across `vox-cli` (telemetry spooling, compiler pipeline, skill installation) and `vox-populi` (mesh gossip sync) to emit gamification events to `vox-gamify::event_router::route_event_auto_user`. Rewards are defined in `crates/vox-gamify/src/reward_policy.rs` and persistent metrics are recorded in `vox_hardened.db`.

**Tech Stack:** Rust (serde_json, tokio), `vox-db` Codex connection, `vox-gamify` reward engine.

---

## Task 1: Map Base Rewards & Unit Tests

**Files:**
- Modify: [reward_policy.rs](file:///c:/Users/Owner/vox/crates/vox-gamify/src/reward_policy.rs)

- [ ] **Step 1: Map the 4 new base rewards in `base_reward()`**

Add these match arms to the `base_reward` lookup inside `crates/vox-gamify/src/reward_policy.rs`:
```rust
        // ── Community / Telemetry / Contribution Events ────────────────────
        "telemetry_shared" => BaseReward::new(20, 4),
        "skill_published" => BaseReward::new(100, 20),
        "skill_gossiped" => BaseReward::with_lumens(150, 30, 15),
        "vox_feature_milestone" => BaseReward::new(40, 8),
```

- [ ] **Step 2: Add unit tests under `mod tests` in `reward_policy.rs`**

Add a test case to assert correct mapping:
```rust
    #[test]
    fn base_reward_for_community_and_contributions() {
        let telemetry = base_reward("telemetry_shared");
        assert_eq!(telemetry.xp, 20);
        assert_eq!(telemetry.crystals, 4);

        let skill_pub = base_reward("skill_published");
        assert_eq!(skill_pub.xp, 100);
        assert_eq!(skill_pub.crystals, 20);

        let skill_gossip = base_reward("skill_gossiped");
        assert_eq!(skill_gossip.xp, 150);
        assert_eq!(skill_gossip.crystals, 30);
        assert_eq!(skill_gossip.lumens, 15);

        let milestone = base_reward("vox_feature_milestone");
        assert_eq!(milestone.xp, 40);
        assert_eq!(milestone.crystals, 8);
    }
```

- [ ] **Step 3: Run unit tests**
```bash
cargo test -p vox-gamify reward_policy::tests
```

- [ ] **Step 4: Commit**
`git add crates/vox-gamify/src/reward_policy.rs`
`git commit -m "feat(ludus): map base rewards and write tests for contribution events"`

---

## Task 2: Hook Telemetry Upload success

**Files:**
- Modify: [telemetry_spool.rs](file:///c:/Users/Owner/vox/crates/vox-cli/src/telemetry_spool.rs)

- [ ] **Step 1: Hook event emit when HTTP upload returns 2xx success**

Inside `upload_pending`, gate the gamification call behind `#[cfg(feature = "vox-gamify")]`:
```rust
        let status = resp.status();
        if status.is_success() {
            ack(&p)?;
            ok += 1;

            #[cfg(feature = "vox-gamify")]
            {
                if let Ok(db) = vox_db::Codex::connect_default().await {
                    let ev = serde_json::json!({
                        "type": "telemetry_shared",
                        "source": "vox-telemetry",
                        "payload": { "bytes_shared": raw.len() },
                    });
                    if let Err(e) = vox_gamify::event_router::route_event_auto_user(&db, &ev).await {
                        tracing::debug!(error = %e, "failed to route telemetry_shared event");
                    }
                }
            }
        }
```

- [ ] **Step 2: Add integration test**

Verify that uploading telemetry spools and triggers `telemetry_shared` reward. Add test inside `crates/vox-gamify/tests/gamify_integration_test.rs`:
```rust
#[tokio::test]
async fn test_telemetry_shared_event_routing() {
    let db = vox_db::VoxDb::open_memory().await.expect("db");
    vox_gamify::db::apply_ludus_migrations(&db).await.expect("migrations");
    let ev = serde_json::json!({
        "type": "telemetry_shared",
        "source": "vox-telemetry",
        "payload": { "bytes_shared": 1024 },
    });
    let res = vox_gamify::event_router::route_event_auto_user(&db, &ev).await.expect("route");
    let rw = res.reward.expect("reward");
    assert_eq!(rw.xp, 20);
    assert_eq!(rw.crystals, 4);
}
```

- [ ] **Step 3: Run integration test**
```bash
cargo test -p vox-gamify --test gamify_integration_test test_telemetry_shared_event_routing
```

- [ ] **Step 4: Commit**
`git add crates/vox-cli/src/telemetry_spool.rs crates/vox-gamify/tests/gamify_integration_test.rs`
`git commit -m "feat(telemetry): hook telemetry upload success with gamify reward"`

---

## Task 3: Hook Successful AST Compilation Primitives

**Files:**
- Modify: [pipeline.rs](file:///c:/Users/Owner/vox/crates/vox-cli/src/pipeline.rs)

- [ ] **Step 1: Scan compiler output source for primitives in `run_frontend_str_with_options`**

Verify compile success (`!res.has_errors()`) and scan code for `@remote`, `@durable`, and `actor`. Use a background task or thread fallback to execute without blocking the compile thread:
```rust
    match vox_compiler::pipeline::run_frontend_str_with_options(source, &file_path, options) {
        Ok(res) => {
            // Check for gamified features if compile succeeded and there are no typeck/HIR errors
            if !res.diagnostics.iter().any(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error) {
                #[cfg(feature = "vox-gamify")]
                {
                    let source_str = source.to_string();
                    let trigger_milestones = move || {
                        let keywords = [("@remote", "@remote"), ("@durable", "@durable"), ("actor", "actor")];
                        for &(kw, feature_name) in &keywords {
                            if source_str.contains(kw) {
                                match tokio::runtime::Handle::try_current() {
                                    Ok(handle) => {
                                        handle.spawn(async move {
                                            if let Ok(db) = vox_db::Codex::connect_default().await {
                                                let ev = serde_json::json!({
                                                    "type": "vox_feature_milestone",
                                                    "source": "vox-compiler",
                                                    "payload": { "feature": feature_name },
                                                });
                                                let _ = vox_gamify::event_router::route_event_auto_user(&db, &ev).await;
                                            }
                                        });
                                    }
                                    Err(_) => {
                                        std::thread::spawn(move || {
                                            if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                                                .enable_all()
                                                .build()
                                            {
                                                rt.block_on(async {
                                                    if let Ok(db) = vox_db::Codex::connect_default().await {
                                                        let ev = serde_json::json!({
                                                            "type": "vox_feature_milestone",
                                                            "source": "vox-compiler",
                                                            "payload": { "feature": feature_name },
                                                        });
                                                        let _ = vox_gamify::event_router::route_event_auto_user(&db, &ev).await;
                                                    }
                                                });
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    };
                    trigger_milestones();
                }
            }
            Ok(FrontendResult {
                module: res.module,
                hir: res.hir,
                diagnostics: res.diagnostics,
                source: res.source,
            })
        }
```

- [ ] **Step 2: Add integration test**

Add test to `crates/vox-gamify/tests/gamify_integration_test.rs`:
```rust
#[tokio::test]
async fn test_vox_feature_milestone_event_routing() {
    let db = vox_db::VoxDb::open_memory().await.expect("db");
    vox_gamify::db::apply_ludus_migrations(&db).await.expect("migrations");
    let ev = serde_json::json!({
        "type": "vox_feature_milestone",
        "source": "vox-compiler",
        "payload": { "feature": "actor" },
    });
    let res = vox_gamify::event_router::route_event_auto_user(&db, &ev).await.expect("route");
    let rw = res.reward.expect("reward");
    assert_eq!(rw.xp, 40);
    assert_eq!(rw.crystals, 8);
}
```

- [ ] **Step 3: Run integration test**
```bash
cargo test -p vox-gamify --test gamify_integration_test test_vox_feature_milestone_event_routing
```

- [ ] **Step 4: Commit**
`git add crates/vox-cli/src/pipeline.rs crates/vox-gamify/tests/gamify_integration_test.rs`
`git commit -m "feat(compiler): trigger language milestone reward on successful compile of actor/@remote/@durable"`

---

## Task 4: Hook Skill Discovery / Installation Catalog Events

**Files:**
- Modify: [registry.rs](file:///c:/Users/Owner/vox/crates/vox-cli/src/commands/extras/ars/registry.rs)

- [ ] **Step 1: Hook event emit when external skill successfully installs**

In `install_external_skills` when `install_bundle` completes with newly installed status:
```rust
        match registry.install_bundle(&ext.bundle).await {
            Ok(res) if !res.already_installed => {
                installed += 1;
                #[cfg(feature = "vox-gamify")]
                {
                    if let Ok(db) = vox_db::Codex::connect_default().await {
                        let ev = serde_json::json!({
                            "type": "skill_published",
                            "source": "vox-skills",
                            "payload": { "skill_name": ext.bundle.manifest.id.clone() },
                        });
                        let _ = vox_gamify::event_router::route_event_auto_user(&db, &ev).await;
                    }
                }
            }
```

- [ ] **Step 2: Add integration test**

Add test to `crates/vox-gamify/tests/gamify_integration_test.rs`:
```rust
#[tokio::test]
async fn test_skill_published_event_routing() {
    let db = vox_db::VoxDb::open_memory().await.expect("db");
    vox_gamify::db::apply_ludus_migrations(&db).await.expect("migrations");
    let ev = serde_json::json!({
        "type": "skill_published",
        "source": "vox-skills",
        "payload": { "skill_name": "my-skill" },
    });
    let res = vox_gamify::event_router::route_event_auto_user(&db, &ev).await.expect("route");
    let rw = res.reward.expect("reward");
    assert_eq!(rw.xp, 100);
    assert_eq!(rw.crystals, 20);
}
```

- [ ] **Step 3: Run integration test**
```bash
cargo test -p vox-gamify --test gamify_integration_test test_skill_published_event_routing
```

- [ ] **Step 4: Commit**
`git add crates/vox-cli/src/commands/extras/ars/registry.rs crates/vox-gamify/tests/gamify_integration_test.rs`
`git commit -m "feat(skills): hook skill discover publish with gamify reward"`

---

## Task 5: Hook Peer Gossip Sync Events (Mesh)

**Files:**
- Modify: [Cargo.toml](file:///c:/Users/Owner/vox/crates/vox-populi/Cargo.toml)
- Modify: [mod.rs](file:///c:/Users/Owner/vox/crates/vox-populi/src/transport/mod.rs)

- [ ] **Step 1: Add optional dependency and feature gate to `vox-populi`**

Update `crates/vox-populi/Cargo.toml`:
```toml
[features]
...
gamify = ["dep:vox-gamify"]

[dependencies]
...
vox-gamify = { workspace = true, optional = true }
```

- [ ] **Step 2: Emit `skill_gossiped` when gossip adds a new mesh peer**

In `crates/vox-populi/src/transport/mod.rs` inside the federation gossip loop:
```rust
                                    } else {
                                        federated.push(peer_entry.clone());

                                        #[cfg(feature = "gamify")]
                                        {
                                            let peer_id = peer_entry.scope_id.clone();
                                            tokio::spawn(async move {
                                                if let Ok(db) = vox_db::Codex::connect_default().await {
                                                    let ev = serde_json::json!({
                                                        "type": "skill_gossiped",
                                                        "source": "vox-populi",
                                                        "payload": { "peer_id": peer_id },
                                                    });
                                                    let _ = vox_gamify::event_router::route_event_auto_user(&db, &ev).await;
                                                }
                                            });
                                        }
                                    }
```

- [ ] **Step 3: Add integration test**

Add test to `crates/vox-gamify/tests/gamify_integration_test.rs`:
```rust
#[tokio::test]
async fn test_skill_gossiped_event_routing() {
    let db = vox_db::VoxDb::open_memory().await.expect("db");
    vox_gamify::db::apply_ludus_migrations(&db).await.expect("migrations");
    let ev = serde_json::json!({
        "type": "skill_gossiped",
        "source": "vox-populi",
        "payload": { "peer_id": "peer-node-1" },
    });
    let res = vox_gamify::event_router::route_event_auto_user(&db, &ev).await.expect("route");
    let rw = res.reward.expect("reward");
    assert_eq!(rw.xp, 150);
    assert_eq!(rw.crystals, 30);
    assert_eq!(rw.lumens, 15);
}
```

- [ ] **Step 4: Run integration test**
```bash
cargo test -p vox-gamify --test gamify_integration_test test_skill_gossiped_event_routing
```

- [ ] **Step 5: Commit**
`git add crates/vox-populi/Cargo.toml crates/vox-populi/src/transport/mod.rs crates/vox-gamify/tests/gamify_integration_test.rs`
`git commit -m "feat(populi): hook peer gossip sync with gamify reward"`

---

## Exit Criteria

- [ ] All 4 new event types map to correct XP, Crystals, and Lumens in `base_reward()`.
- [ ] Successful telemetry uploads route a `telemetry_shared` event.
- [ ] Compiler successes scanning `@remote`, `@durable`, or `actor` route a `vox_feature_milestone` event.
- [ ] Newly discovered external skills route a `skill_published` event.
- [ ] Populi federation mesh additions route a `skill_gossiped` event.
- [ ] All integration tests pass successfully.
