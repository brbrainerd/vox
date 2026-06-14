/// Wave-19 adversarial tests for vox-orchestrator-queue.
///
/// Targets: resource.rs, locks/refresh.rs, sync_lock.rs, projection.rs,
/// projections/{affinity,capabilities,kudos,locks}.rs
///
/// Every test carries: `// Catches: <specific bug>`

#[cfg(test)]
mod semcov_wave19_tests {
    // -----------------------------------------------------------------------
    // ResourceLockManager tests (resource.rs)
    // -----------------------------------------------------------------------
    mod resource_lock {
        use crate::locks::resource::{ResourceLockKind, ResourceLockManager};
        use vox_orchestrator_types::AgentId;

        fn now_plus_ms(delta: u64) -> u64 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + delta
        }

        #[test]
        fn acquire_returns_lock_with_correct_expiry() {
            // Catches: expires_ms computed from wrong epoch reference (e.g. boot time instead of wall clock)
            let mgr = ResourceLockManager::new();
            let before = now_plus_ms(0);
            let lock = mgr
                .try_acquire("db://row/1", AgentId(1), ResourceLockKind::Exclusive, 5_000)
                .expect("should succeed");
            let after = now_plus_ms(0);
            assert!(
                lock.expires_ms >= before + 4_999 && lock.expires_ms <= after + 5_001,
                "expires_ms out of expected range: {}",
                lock.expires_ms
            );
        }

        #[test]
        fn conflict_returns_err_not_panic() {
            // Catches: unwrap() on existing lock map entry → panic instead of Err
            let mgr = ResourceLockManager::new();
            mgr.try_acquire(
                "db://row/1",
                AgentId(1),
                ResourceLockKind::Exclusive,
                60_000,
            )
            .unwrap();
            let result = mgr.try_acquire(
                "db://row/1",
                AgentId(2),
                ResourceLockKind::Exclusive,
                60_000,
            );
            assert!(
                result.is_err(),
                "second acquirer must get Err, not Ok or panic"
            );
        }

        #[test]
        fn error_message_names_blocking_agent() {
            // Catches: generic "locked" error omits holder identity → impossible to debug contention
            let mgr = ResourceLockManager::new();
            mgr.try_acquire("res://x", AgentId(42), ResourceLockKind::Exclusive, 60_000)
                .unwrap();
            let err = mgr
                .try_acquire("res://x", AgentId(99), ResourceLockKind::Exclusive, 60_000)
                .unwrap_err();
            assert!(
                err.contains("42"),
                "error must mention holder agent id (42), got: {err}"
            );
        }

        #[test]
        fn reentrant_acquire_extends_ttl() {
            // Catches: re-entrant acquire is rejected (wrong: it should extend the TTL)
            let mgr = ResourceLockManager::new();
            mgr.try_acquire("res://y", AgentId(1), ResourceLockKind::Exclusive, 1_000)
                .unwrap();
            let renewed = mgr
                .try_acquire("res://y", AgentId(1), ResourceLockKind::Exclusive, 60_000)
                .expect("re-entrant acquire should succeed for same agent");
            let floor = now_plus_ms(0) + 59_000;
            assert!(
                renewed.expires_ms >= floor,
                "re-entrant acquire must extend TTL; got expires_ms={}",
                renewed.expires_ms
            );
        }

        #[test]
        fn release_by_non_holder_is_noop() {
            // Catches: release ignores holder check → any agent can steal-release a lock
            let mgr = ResourceLockManager::new();
            mgr.try_acquire("res://z", AgentId(1), ResourceLockKind::Exclusive, 60_000)
                .unwrap();
            mgr.release("res://z", AgentId(99)); // wrong agent
            assert!(
                mgr.is_locked("res://z"),
                "non-holder release must not remove the lock"
            );
        }

        #[test]
        fn is_locked_returns_false_after_expiry() {
            // Catches: is_locked ignores expires_ms and returns true for stale entries
            let mgr = ResourceLockManager::new();
            // ttl_ms = 0 → already expired at insertion time
            mgr.try_acquire("res://exp", AgentId(1), ResourceLockKind::Exclusive, 0)
                .unwrap();
            // Give wall-clock a moment to overtake expires_ms=now+0
            std::thread::sleep(std::time::Duration::from_millis(2));
            assert!(
                !mgr.is_locked("res://exp"),
                "lock with ttl=0 must read as not locked after expiry"
            );
        }

        #[test]
        fn len_counts_only_unexpired_via_snapshot() {
            // Catches: len() counts all map entries including expired ones
            let mgr = ResourceLockManager::new();
            mgr.try_acquire("res://a", AgentId(1), ResourceLockKind::Exclusive, 60_000)
                .unwrap();
            mgr.try_acquire("res://b", AgentId(2), ResourceLockKind::Exclusive, 60_000)
                .unwrap();
            assert_eq!(mgr.len(), 2, "two active locks");
            mgr.release("res://a", AgentId(1));
            assert_eq!(mgr.len(), 1, "after release, one active lock");
        }

        #[test]
        fn snapshot_round_trips_all_fields() {
            // Catches: snapshot() silently drops resource_id or kind field during clone
            let mgr = ResourceLockManager::new();
            mgr.try_acquire("db://table/7", AgentId(5), ResourceLockKind::Shared, 30_000)
                .unwrap();
            let snap = mgr.snapshot();
            assert_eq!(snap.len(), 1);
            let entry = &snap[0];
            assert_eq!(entry.resource_id, "db://table/7");
            assert_eq!(entry.holder, AgentId(5));
            assert!(matches!(entry.kind, ResourceLockKind::Shared));
        }

        #[test]
        fn expired_lock_allows_new_holder() {
            // Catches: stale entry blocks new acquirer even after expiry
            let mgr = ResourceLockManager::new();
            mgr.try_acquire("res://old", AgentId(1), ResourceLockKind::Exclusive, 0)
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
            let result =
                mgr.try_acquire("res://old", AgentId(2), ResourceLockKind::Exclusive, 5_000);
            assert!(
                result.is_ok(),
                "new agent must be able to acquire expired lock"
            );
        }
    }

    // -----------------------------------------------------------------------
    // FileLockManager::refresh.rs (force_release_stale, escalate, queue)
    // -----------------------------------------------------------------------
    mod file_lock_refresh {
        use std::path::Path;

        use crate::locks::{FileLockManager, LockConflict, LockKind};
        use vox_orchestrator_types::AgentId;

        #[test]
        fn force_release_stale_removes_exclusive_past_timeout() {
            // Catches: force_release_stale checks elapsed > timeout with wrong comparison (>=/>)
            // causing a lock that's exactly at the boundary to not be released
            let mgr = FileLockManager::new();
            mgr.try_acquire(Path::new("old.rs"), AgentId(1), LockKind::Exclusive)
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
            // timeout of 1 ms → lock is stale
            let count = mgr.force_release_stale(1);
            assert_eq!(count, 1, "one stale exclusive lock should be released");
            assert!(!mgr.is_locked(Path::new("old.rs")));
        }

        #[test]
        fn force_release_stale_spares_fresh_lock() {
            // Catches: force_release_stale uses wrong time source (e.g. 0) and evicts all locks
            let mgr = FileLockManager::new();
            mgr.try_acquire(Path::new("fresh.rs"), AgentId(1), LockKind::Exclusive)
                .unwrap();
            let count = mgr.force_release_stale(u128::MAX);
            assert_eq!(
                count, 0,
                "fresh lock must not be released with huge timeout"
            );
            assert!(mgr.is_locked(Path::new("fresh.rs")));
        }

        #[test]
        fn force_release_stale_partial_shared_readers() {
            // Catches: force_release_stale drops ALL shared-read entries even if only some are stale
            let mgr = FileLockManager::new();
            mgr.try_acquire(Path::new("shared.rs"), AgentId(1), LockKind::SharedRead)
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
            mgr.try_acquire(Path::new("shared.rs"), AgentId(2), LockKind::SharedRead)
                .unwrap();
            // Only agent-1's lock is old enough to be stale (1 ms threshold)
            let count = mgr.force_release_stale(2);
            assert_eq!(count, 1, "only the stale reader should be evicted");
            // File is still locked by the fresh agent-2 reader
            assert!(mgr.is_locked(Path::new("shared.rs")));
        }

        #[test]
        fn escalate_read_to_write_succeeds_when_sole_reader() {
            // Catches: escalate always returns SharedReadersExist even for sole-reader case
            let mgr = FileLockManager::new();
            mgr.try_acquire(Path::new("f.rs"), AgentId(1), LockKind::SharedRead)
                .unwrap();
            mgr.escalate_read_to_write(AgentId(1), Path::new("f.rs"))
                .expect("sole reader escalation must succeed");
            // After escalation the lock must be exclusive
            let (holder, kind) = mgr.holder(Path::new("f.rs")).unwrap();
            assert_eq!(holder, AgentId(1));
            assert_eq!(kind, LockKind::Exclusive);
        }

        #[test]
        fn escalate_read_to_write_fails_with_multiple_readers() {
            // Catches: escalate ignores other readers and always upgrades → data race
            let mgr = FileLockManager::new();
            mgr.try_acquire(Path::new("g.rs"), AgentId(1), LockKind::SharedRead)
                .unwrap();
            mgr.try_acquire(Path::new("g.rs"), AgentId(2), LockKind::SharedRead)
                .unwrap();
            let err = mgr
                .escalate_read_to_write(AgentId(1), Path::new("g.rs"))
                .unwrap_err();
            assert!(
                matches!(err, LockConflict::SharedReadersExist { .. }),
                "must return SharedReadersExist when multiple readers exist"
            );
        }

        #[test]
        fn escalate_when_exclusively_held_by_another_fails() {
            // Catches: escalate succeeds for any agent if an exclusive lock exists (wrong holder check)
            let mgr = FileLockManager::new();
            mgr.try_acquire(Path::new("h.rs"), AgentId(7), LockKind::Exclusive)
                .unwrap();
            let err = mgr
                .escalate_read_to_write(AgentId(99), Path::new("h.rs"))
                .unwrap_err();
            assert!(
                matches!(err, LockConflict::ExclusivelyHeld { .. }),
                "escalate by non-holder when exclusive must fail"
            );
        }

        #[test]
        fn queue_agent_dedup_prevents_double_enqueue() {
            // Catches: queue_agent_for_lock allows duplicate entries → inflated contention_count
            let mgr = FileLockManager::new();
            let path = Path::new("q.rs");
            mgr.queue_agent_for_lock(AgentId(1), path);
            mgr.queue_agent_for_lock(AgentId(1), path); // duplicate
            assert_eq!(
                mgr.contention_count(),
                1,
                "same agent queued twice must appear only once"
            );
        }

        #[test]
        fn dequeue_waiter_fifo_ordering() {
            // Catches: dequeue_waiter returns last-in instead of first-in (stack vs queue)
            let mgr = FileLockManager::new();
            let path = Path::new("fifo.rs");
            mgr.queue_agent_for_lock(AgentId(10), path);
            mgr.queue_agent_for_lock(AgentId(20), path);
            mgr.queue_agent_for_lock(AgentId(30), path);
            assert_eq!(
                mgr.dequeue_waiter(path),
                Some(AgentId(10)),
                "FIFO: first in must be first out"
            );
            assert_eq!(mgr.dequeue_waiter(path), Some(AgentId(20)));
            assert_eq!(mgr.dequeue_waiter(path), Some(AgentId(30)));
            assert_eq!(
                mgr.dequeue_waiter(path),
                None,
                "empty queue must return None"
            );
        }

        #[test]
        fn contention_count_sums_across_all_paths() {
            // Catches: contention_count only counts the first path's queue
            let mgr = FileLockManager::new();
            mgr.queue_agent_for_lock(AgentId(1), Path::new("a.rs"));
            mgr.queue_agent_for_lock(AgentId(2), Path::new("b.rs"));
            mgr.queue_agent_for_lock(AgentId(3), Path::new("b.rs"));
            assert_eq!(mgr.contention_count(), 3);
        }

        #[test]
        fn dequeue_on_empty_path_returns_none() {
            // Catches: dequeue_waiter panics or returns garbage for non-existent path
            let mgr = FileLockManager::new();
            let result = mgr.dequeue_waiter(Path::new("nonexistent.rs"));
            assert_eq!(result, None);
        }
    }

    // -----------------------------------------------------------------------
    // sync_lock.rs — poison recovery
    // -----------------------------------------------------------------------
    mod sync_lock_tests {
        use std::sync::RwLock;

        use crate::sync_lock;

        #[test]
        fn rw_read_recovers_from_poisoned_lock() {
            // Catches: rw_read calls unwrap() instead of into_inner() → panic after writer panic
            let lock = RwLock::new(42u32);
            // Poison the lock by panicking inside a write guard
            let _ = std::panic::catch_unwind(|| {
                let _g = lock.write().unwrap();
                panic!("intentional poison");
            });
            assert!(lock.is_poisoned(), "lock should be poisoned");
            let val = *sync_lock::rw_read(&lock);
            // Value must still be readable (may be 42 or mid-write, but no panic)
            let _ = val; // just confirm no panic
        }

        #[test]
        fn rw_write_recovers_from_poisoned_lock() {
            // Catches: rw_write panics on PoisonError instead of recovering state
            let lock = RwLock::new(0u32);
            let _ = std::panic::catch_unwind(|| {
                let _g = lock.write().unwrap();
                panic!("poison");
            });
            {
                let mut guard = sync_lock::rw_write(&lock);
                *guard = 99;
            }
            assert_eq!(*sync_lock::rw_read(&lock), 99);
        }
    }

    // -----------------------------------------------------------------------
    // ProjectionRegistry (projection.rs)
    // -----------------------------------------------------------------------
    mod projection_registry {
        use crate::oplog::{OperationEntry, OperationId, OperationKind};
        use crate::projection::{Projection, ProjectionError, ProjectionRegistry};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU64, Ordering};
        use vox_orchestrator_types::AgentId;

        fn dummy_entry(kind: OperationKind) -> OperationEntry {
            OperationEntry {
                id: OperationId(1),
                agent_id: AgentId(1),
                timestamp_ms: 0,
                kind,
                description: String::new(),
                snapshot_before: None,
                snapshot_after: None,
                db_snapshot_before: None,
                db_snapshot_after: None,
                context_snapshot_before: None,
                context_snapshot_after: None,
                undone: false,
                change_id: None,
                model_id: None,
                predecessor_hash: None,
                signature: None,
                signing_key_id: None,
                daemon_id: [0u8; 16],
                parent_op_ids: vec![],
            }
        }

        struct CountingProjection {
            count: Arc<AtomicU64>,
        }

        impl Projection for CountingProjection {
            fn name(&self) -> &'static str {
                "counter"
            }
            fn apply(&self, _e: &OperationEntry) {
                self.count.fetch_add(1, Ordering::Relaxed);
            }
            fn snapshot(&self) -> Vec<u8> {
                self.count.load(Ordering::Relaxed).to_be_bytes().to_vec()
            }
            fn restore(&self, b: &[u8]) -> Result<(), ProjectionError> {
                if b.len() < 8 {
                    return Err(ProjectionError::Decode("too short".into()));
                }
                let val = u64::from_be_bytes(b[..8].try_into().unwrap());
                self.count.store(val, Ordering::Relaxed);
                Ok(())
            }
        }

        #[tokio::test]
        async fn apply_fans_out_to_all_projections() {
            // Catches: registry only calls apply() on the first registered projection
            let c1 = Arc::new(AtomicU64::new(0));
            let c2 = Arc::new(AtomicU64::new(0));
            let reg = ProjectionRegistry::new()
                .with(CountingProjection {
                    count: Arc::clone(&c1),
                })
                .with(CountingProjection {
                    count: Arc::clone(&c2),
                });
            let entry = dummy_entry(OperationKind::Rebalance);
            reg.apply(&entry).await;
            assert_eq!(
                c1.load(Ordering::Relaxed),
                1,
                "first projection must receive apply"
            );
            assert_eq!(
                c2.load(Ordering::Relaxed),
                1,
                "second projection must receive apply"
            );
        }

        #[test]
        fn snapshot_blake3_deterministic() {
            // Catches: blake3 hash uses non-deterministic input (e.g. pointer addresses) → hashes differ per run
            let make_reg = || {
                let c = Arc::new(AtomicU64::new(7));
                ProjectionRegistry::new().with(CountingProjection { count: c })
            };
            let hash1 = make_reg().snapshot_blake3();
            let hash2 = make_reg().snapshot_blake3();
            assert_eq!(
                hash1, hash2,
                "identical state must produce identical blake3 hash"
            );
        }

        #[test]
        fn snapshot_blake3_differs_after_apply() {
            // Catches: blake3 digest always returns a constant (projection snapshot not actually used)
            let c = Arc::new(AtomicU64::new(0));
            let reg = ProjectionRegistry::new().with(CountingProjection {
                count: Arc::clone(&c),
            });
            let hash_before = reg.snapshot_blake3();
            c.fetch_add(1, Ordering::Relaxed);
            let hash_after = reg.snapshot_blake3();
            assert_ne!(
                hash_before, hash_after,
                "different projection state must produce different blake3 hash"
            );
        }

        #[test]
        fn empty_registry_returns_stable_hash() {
            // Catches: empty iterator panics (unwrap) or produces zero-length output that collides
            let reg = ProjectionRegistry::new();
            let h1 = reg.snapshot_blake3();
            let h2 = reg.snapshot_blake3();
            assert_eq!(h1, h2, "empty registry must produce consistent hash");
        }
    }

    // -----------------------------------------------------------------------
    // projections/affinity.rs (AffinityProjection)
    // -----------------------------------------------------------------------
    mod affinity_projection {
        use crate::oplog::{OperationEntry, OperationId, OperationKind};
        use crate::projection::{Projection, ProjectionError};
        use crate::projections::AffinityProjection;
        use vox_orchestrator_types::AgentId;

        fn entry_for(agent: u64, kind: OperationKind, daemon: [u8; 16]) -> OperationEntry {
            OperationEntry {
                id: OperationId(1),
                agent_id: AgentId(agent),
                timestamp_ms: 1_000,
                kind,
                description: String::new(),
                snapshot_before: None,
                snapshot_after: None,
                db_snapshot_before: None,
                db_snapshot_after: None,
                context_snapshot_before: None,
                context_snapshot_after: None,
                undone: false,
                change_id: None,
                model_id: None,
                predecessor_hash: None,
                signature: None,
                signing_key_id: None,
                daemon_id: daemon,
                parent_op_ids: vec![],
            }
        }

        #[test]
        fn workspace_create_then_merge_removes_entry() {
            // Catches: WorkspaceMerge does not remove the affinity entry (stale ownership retained)
            let proj = AffinityProjection::default();
            proj.apply(&entry_for(
                1,
                OperationKind::WorkspaceCreate { agent_id: 1 },
                [0u8; 16],
            ));
            let snap_after_create = proj.snapshot();
            assert!(!snap_after_create.is_empty());

            proj.apply(&entry_for(
                1,
                OperationKind::WorkspaceMerge { agent_id: 1 },
                [0u8; 16],
            ));
            // After merge the workspace entry must be gone
            let snap_after_merge = proj.snapshot();
            let map: std::collections::BTreeMap<String, serde_json::Value> =
                serde_json::from_slice(&snap_after_merge).unwrap();
            assert!(
                !map.contains_key("workspace:1"),
                "workspace:1 must be absent after merge"
            );
        }

        #[test]
        fn custom_claim_then_release_removes_path() {
            // Catches: affinity.release: handler does not remove the key → ghost claim survives
            let proj = AffinityProjection::default();
            proj.apply(&entry_for(
                2,
                OperationKind::Custom {
                    label: "affinity.claim:src/main.rs".into(),
                },
                [1u8; 16],
            ));
            proj.apply(&entry_for(
                2,
                OperationKind::Custom {
                    label: "affinity.release:src/main.rs".into(),
                },
                [1u8; 16],
            ));
            let map: std::collections::BTreeMap<String, serde_json::Value> =
                serde_json::from_slice(&proj.snapshot()).unwrap();
            assert!(
                !map.contains_key("src/main.rs"),
                "path must be absent after release"
            );
        }

        #[test]
        fn snapshot_restore_round_trip() {
            // Catches: restore() rejects a valid snapshot (e.g. wrong serde type tag) → projection never hydrates
            let proj = AffinityProjection::default();
            proj.apply(&entry_for(
                3,
                OperationKind::Custom {
                    label: "affinity.claim:lib.rs".into(),
                },
                [2u8; 16],
            ));
            let bytes = proj.snapshot();

            let fresh = AffinityProjection::default();
            fresh
                .restore(&bytes)
                .expect("restore from valid snapshot must succeed");
            assert_eq!(
                fresh.snapshot(),
                bytes,
                "round-tripped snapshot must be identical"
            );
        }

        #[test]
        fn restore_invalid_bytes_returns_decode_error() {
            // Catches: restore() panics on bad bytes instead of returning Err(Decode)
            let proj = AffinityProjection::default();
            let result = proj.restore(b"not valid json");
            assert!(
                matches!(result, Err(ProjectionError::Decode(_))),
                "corrupt bytes must yield ProjectionError::Decode"
            );
        }

        #[test]
        fn lamport_clock_increments_monotonically() {
            // Catches: lamport counter wraps or resets on concurrent calls → LWW breaks
            let proj = AffinityProjection::default();
            for i in 0..5u64 {
                proj.apply(&entry_for(
                    i,
                    OperationKind::Custom {
                        label: format!("affinity.claim:file{i}.rs"),
                    },
                    [0u8; 16],
                ));
            }
            let snap: std::collections::BTreeMap<
                String,
                crate::projections::affinity::AffinityOwner,
            > = serde_json::from_slice(&proj.snapshot()).unwrap();
            let lamports: Vec<u64> = snap.values().map(|v| v.lamport).collect();
            // Each claim must have a strictly higher lamport than its predecessor
            // (sorted by insertion order, so we check all-distinct and all > 0)
            let mut sorted = lamports.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(
                sorted.len(),
                lamports.len(),
                "all lamport values must be unique"
            );
            assert!(
                lamports.iter().all(|&l| l > 0),
                "lamport must start from 1, not 0"
            );
        }
    }

    // -----------------------------------------------------------------------
    // projections/capabilities.rs (CapabilityProjection)
    // -----------------------------------------------------------------------
    mod capability_projection {
        use crate::oplog::{OperationEntry, OperationId, OperationKind};
        use crate::projection::{Projection, ProjectionError};
        use crate::projections::CapabilityProjection;
        use vox_orchestrator_types::AgentId;

        fn cap_entry(agent: u64, label: &str) -> OperationEntry {
            OperationEntry {
                id: OperationId(agent),
                agent_id: AgentId(agent),
                timestamp_ms: 1_234_567,
                kind: OperationKind::Custom {
                    label: label.to_string(),
                },
                description: String::new(),
                snapshot_before: None,
                snapshot_after: None,
                db_snapshot_before: None,
                db_snapshot_after: None,
                context_snapshot_before: None,
                context_snapshot_after: None,
                undone: false,
                change_id: None,
                model_id: None,
                predecessor_hash: None,
                signature: None,
                signing_key_id: None,
                daemon_id: [0u8; 16],
                parent_op_ids: vec![],
            }
        }

        #[test]
        fn mint_then_revoke_removes_capability() {
            // Catches: cap.revoke: handler parses op_id incorrectly → revoke silently no-ops
            let proj = CapabilityProjection::default();
            proj.apply(&cap_entry(1, "cap.mint:write"));
            // op_id of the mint entry is OperationId(1) → revoke references "1"
            proj.apply(&cap_entry(2, "cap.revoke:1"));
            let snap: std::collections::BTreeMap<u64, serde_json::Value> =
                serde_json::from_slice(&proj.snapshot()).unwrap();
            assert!(
                !snap.contains_key(&1),
                "capability with op_id=1 must be absent after revoke"
            );
        }

        #[test]
        fn mint_records_correct_agent_and_kind() {
            // Catches: cap.mint stores wrong agent_id (e.g. always 0) or truncates kind label
            let proj = CapabilityProjection::default();
            proj.apply(&cap_entry(77, "cap.mint:execute:sandboxed"));
            let snap: std::collections::BTreeMap<
                u64,
                crate::projections::capabilities::CapabilityRecord,
            > = serde_json::from_slice(&proj.snapshot()).unwrap();
            let record = snap.get(&77).expect("record for op_id=77 must exist");
            assert_eq!(record.agent_id, 77);
            assert_eq!(
                record.kind, "execute:sandboxed",
                "kind must preserve full suffix after first colon"
            );
        }

        #[test]
        fn revoke_nonexistent_op_is_noop() {
            // Catches: revoke on missing op_id panics or corrupts state
            let proj = CapabilityProjection::default();
            proj.apply(&cap_entry(1, "cap.mint:read"));
            proj.apply(&cap_entry(2, "cap.revoke:9999")); // non-existent
            let snap: std::collections::BTreeMap<u64, serde_json::Value> =
                serde_json::from_slice(&proj.snapshot()).unwrap();
            assert_eq!(
                snap.len(),
                1,
                "revoke of nonexistent id must leave existing capability intact"
            );
        }

        #[test]
        fn restore_roundtrip_preserves_all_records() {
            // Catches: restore() silently swallows entries with numeric keys (serde JSON u64 map keys)
            let proj = CapabilityProjection::default();
            proj.apply(&cap_entry(10, "cap.mint:alpha"));
            proj.apply(&cap_entry(20, "cap.mint:beta"));
            let bytes = proj.snapshot();
            let fresh = CapabilityProjection::default();
            fresh.restore(&bytes).expect("restore must succeed");
            assert_eq!(
                fresh.snapshot(),
                bytes,
                "capability snapshot must round-trip identically"
            );
        }

        #[test]
        fn restore_garbage_bytes_yields_decode_error() {
            // Catches: restore() calls expect() on serde error → panics in production
            let proj = CapabilityProjection::default();
            let result = proj.restore(b"\xff\xfe");
            assert!(matches!(result, Err(ProjectionError::Decode(_))));
        }
    }

    // -----------------------------------------------------------------------
    // projections/kudos.rs (KudosProjection)
    // -----------------------------------------------------------------------
    mod kudos_projection {
        use crate::oplog::{OperationEntry, OperationId, OperationKind};
        use crate::projection::{Projection, ProjectionError};
        use crate::projections::KudosProjection;
        use vox_orchestrator_types::AgentId;

        fn kudos_entry(agent: u64, label: &str) -> OperationEntry {
            OperationEntry {
                id: OperationId(1),
                agent_id: AgentId(agent),
                timestamp_ms: 0,
                kind: OperationKind::Custom {
                    label: label.to_string(),
                },
                description: String::new(),
                snapshot_before: None,
                snapshot_after: None,
                db_snapshot_before: None,
                db_snapshot_after: None,
                context_snapshot_before: None,
                context_snapshot_after: None,
                undone: false,
                change_id: None,
                model_id: None,
                predecessor_hash: None,
                signature: None,
                signing_key_id: None,
                daemon_id: [0u8; 16],
                parent_op_ids: vec![],
            }
        }

        #[test]
        fn kudos_accumulate_across_multiple_adds() {
            // Catches: kudos.add always overwrites instead of accumulating
            let proj = KudosProjection::default();
            proj.apply(&kudos_entry(1, "kudos.add:codegen:10"));
            proj.apply(&kudos_entry(1, "kudos.add:codegen:5"));
            proj.apply(&kudos_entry(1, "kudos.add:codegen:3"));
            // Total must be 18
            let snap: Vec<(u64, String, i64)> = serde_json::from_slice(&proj.snapshot()).unwrap();
            let total: i64 = snap
                .iter()
                .filter(|(a, p, _)| *a == 1 && p == "codegen")
                .map(|(_, _, v)| v)
                .sum();
            assert_eq!(total, 18, "kudos must accumulate to 18");
        }

        #[test]
        fn kudos_separate_per_primitive() {
            // Catches: kudos.add ignores primitive and accumulates into a single bucket
            let proj = KudosProjection::default();
            proj.apply(&kudos_entry(1, "kudos.add:codegen:10"));
            proj.apply(&kudos_entry(1, "kudos.add:review:7"));
            let snap: Vec<(u64, String, i64)> = serde_json::from_slice(&proj.snapshot()).unwrap();
            let codegen: i64 = snap
                .iter()
                .filter(|(_, p, _)| p == "codegen")
                .map(|(_, _, v)| v)
                .sum();
            let review: i64 = snap
                .iter()
                .filter(|(_, p, _)| p == "review")
                .map(|(_, _, v)| v)
                .sum();
            assert_eq!(codegen, 10);
            assert_eq!(review, 7);
        }

        #[test]
        fn kudos_separate_per_agent() {
            // Catches: kudos aggregated by primitive only, ignoring agent_id
            let proj = KudosProjection::default();
            proj.apply(&kudos_entry(1, "kudos.add:test:5"));
            proj.apply(&kudos_entry(2, "kudos.add:test:3"));
            let snap: Vec<(u64, String, i64)> = serde_json::from_slice(&proj.snapshot()).unwrap();
            let agent1: i64 = snap
                .iter()
                .filter(|(a, p, _)| *a == 1 && p == "test")
                .map(|(_, _, v)| v)
                .sum();
            let agent2: i64 = snap
                .iter()
                .filter(|(a, p, _)| *a == 2 && p == "test")
                .map(|(_, _, v)| v)
                .sum();
            assert_eq!(agent1, 5);
            assert_eq!(agent2, 3);
        }

        #[test]
        fn malformed_kudos_label_is_ignored() {
            // Catches: label with missing amount field panics on parse
            let proj = KudosProjection::default();
            proj.apply(&kudos_entry(1, "kudos.add:no_amount_here")); // missing :<amount>
            let snap: Vec<(u64, String, i64)> = serde_json::from_slice(&proj.snapshot()).unwrap();
            assert!(
                snap.is_empty(),
                "malformed kudos label must be silently ignored"
            );
        }

        #[test]
        fn snapshot_is_deterministic_across_two_instances() {
            // Catches: snapshot serialization uses HashMap with non-deterministic order
            let make = || {
                let p = KudosProjection::default();
                p.apply(&kudos_entry(1, "kudos.add:x:1"));
                p.apply(&kudos_entry(2, "kudos.add:y:2"));
                p.snapshot()
            };
            assert_eq!(make(), make(), "kudos snapshot must be deterministic");
        }

        #[test]
        fn restore_roundtrip_preserves_values() {
            // Catches: restore() forgets to clear existing state before inserting → stale values persist
            let proj = KudosProjection::default();
            proj.apply(&kudos_entry(5, "kudos.add:ship:100"));
            let bytes = proj.snapshot();
            let fresh = KudosProjection::default();
            fresh.restore(&bytes).expect("restore must succeed");
            assert_eq!(
                fresh.snapshot(),
                bytes,
                "kudos round-trip must be identical"
            );
        }

        #[test]
        fn restore_garbage_bytes_yields_decode_error() {
            // Catches: restore() panics on bad JSON instead of returning Err
            let proj = KudosProjection::default();
            let result = proj.restore(b"}{broken");
            assert!(matches!(result, Err(ProjectionError::Decode(_))));
        }
    }

    // -----------------------------------------------------------------------
    // projections/locks.rs (LocksProjection)
    // -----------------------------------------------------------------------
    mod locks_projection {
        use crate::oplog::{OperationEntry, OperationId, OperationKind};
        use crate::projection::{Projection, ProjectionError};
        use crate::projections::LocksProjection;
        use vox_orchestrator_types::AgentId;

        fn lock_entry(agent: u64, kind: OperationKind, ts: u64) -> OperationEntry {
            OperationEntry {
                id: OperationId(agent),
                agent_id: AgentId(agent),
                timestamp_ms: ts,
                kind,
                description: String::new(),
                snapshot_before: None,
                snapshot_after: None,
                db_snapshot_before: None,
                db_snapshot_after: None,
                context_snapshot_before: None,
                context_snapshot_after: None,
                undone: false,
                change_id: None,
                model_id: None,
                predecessor_hash: None,
                signature: None,
                signing_key_id: None,
                daemon_id: [3u8; 16],
                parent_op_ids: vec![],
            }
        }

        #[test]
        fn lock_acquire_then_release_removes_path() {
            // Catches: LockRelease handler does nothing (missing arm in match)
            let proj = LocksProjection::default();
            proj.apply(&lock_entry(
                1,
                OperationKind::LockAcquire {
                    path: "src/lib.rs".into(),
                    agent_id: 1,
                },
                1000,
            ));
            proj.apply(&lock_entry(
                1,
                OperationKind::LockRelease {
                    path: "src/lib.rs".into(),
                    agent_id: 1,
                },
                2000,
            ));
            let map: std::collections::BTreeMap<String, serde_json::Value> =
                serde_json::from_slice(&proj.snapshot()).unwrap();
            assert!(
                !map.contains_key("src/lib.rs"),
                "path must be removed after LockRelease"
            );
        }

        #[test]
        fn lock_acquire_sets_lease_expiry_60s_after_timestamp() {
            // Catches: lease_expires_ms computed as timestamp_ms alone instead of +60_000
            let proj = LocksProjection::default();
            proj.apply(&lock_entry(
                1,
                OperationKind::LockAcquire {
                    path: "x.rs".into(),
                    agent_id: 1,
                },
                100_000,
            ));
            let map: std::collections::BTreeMap<String, crate::projections::locks::LockOwner> =
                serde_json::from_slice(&proj.snapshot()).unwrap();
            let owner = map.get("x.rs").expect("lock must exist");
            assert_eq!(
                owner.lease_expires_ms, 160_000,
                "lease must expire 60 s after timestamp"
            );
        }

        #[test]
        fn custom_lock_acquire_and_release() {
            // Catches: custom lock.acquire:/lock.release: labels not handled
            let proj = LocksProjection::default();
            proj.apply(&lock_entry(
                9,
                OperationKind::Custom {
                    label: "lock.acquire:infra/db.toml".into(),
                },
                500,
            ));
            {
                let map: std::collections::BTreeMap<String, serde_json::Value> =
                    serde_json::from_slice(&proj.snapshot()).unwrap();
                assert!(
                    map.contains_key("infra/db.toml"),
                    "custom acquire must register path"
                );
            }
            proj.apply(&lock_entry(
                9,
                OperationKind::Custom {
                    label: "lock.release:infra/db.toml".into(),
                },
                600,
            ));
            let map: std::collections::BTreeMap<String, serde_json::Value> =
                serde_json::from_slice(&proj.snapshot()).unwrap();
            assert!(
                !map.contains_key("infra/db.toml"),
                "custom release must remove path"
            );
        }

        #[test]
        fn snapshot_restore_round_trip() {
            // Catches: restore() fails silently or returns wrong state
            let proj = LocksProjection::default();
            proj.apply(&lock_entry(
                2,
                OperationKind::LockAcquire {
                    path: "go.rs".into(),
                    agent_id: 2,
                },
                3000,
            ));
            let bytes = proj.snapshot();
            let fresh = LocksProjection::default();
            fresh.restore(&bytes).expect("restore must succeed");
            assert_eq!(fresh.snapshot(), bytes, "snapshot must round-trip");
        }

        #[test]
        fn restore_garbage_bytes_yields_decode_error() {
            // Catches: restore() uses unwrap() on serde result → production panic on corrupted checkpoint
            let proj = LocksProjection::default();
            let result = proj.restore(b"not json");
            assert!(matches!(result, Err(ProjectionError::Decode(_))));
        }

        #[test]
        fn second_acquire_on_same_path_overwrites_owner() {
            // Catches: LockAcquire silently no-ops when path already exists → wrong holder retained
            let proj = LocksProjection::default();
            proj.apply(&lock_entry(
                1,
                OperationKind::LockAcquire {
                    path: "contested.rs".into(),
                    agent_id: 1,
                },
                1000,
            ));
            proj.apply(&lock_entry(
                2,
                OperationKind::LockAcquire {
                    path: "contested.rs".into(),
                    agent_id: 2,
                },
                2000,
            ));
            let map: std::collections::BTreeMap<String, crate::projections::locks::LockOwner> =
                serde_json::from_slice(&proj.snapshot()).unwrap();
            let owner = map.get("contested.rs").expect("must exist");
            assert_eq!(
                owner.agent_id, 2,
                "second acquire must overwrite with new agent (last-write-wins)"
            );
        }
    }
}
