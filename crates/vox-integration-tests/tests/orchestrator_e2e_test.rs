#![allow(missing_docs)]

//! Orchestrator integration tests.
//!
//! ## Runtime: multiple Tokio worker threads
//!
//! `#[tokio::test]` defaults to a multi-thread runtime with **one worker**. Paths that call
//! `VoxDb::block_on` inside an active runtime can deadlock that pool; use **`worker_threads >= 2`**.
//!
//! ## Forensic logs
//!
//! Per-test logs are written under `target/e2e-logs/orchestrator_e2e_<test>_<pid>.log` (repo root).
//! Lines include OS thread IDs so parallel `cargo test` runs can be correlated.
//!
//! The status watchdog runs on a **native `std::thread`**, not Tokio, so if all runtime workers
//! stall/deadlock you still get ~3s `watchdog[...]` snapshots (pinning last known queue state).
//!
//! If tests still appear wedged only under high parallelism, try:
//! `cargo test -p vox-integration-tests --test orchestrator_e2e -- --test-threads=1`.

use std::fs::OpenOptions;
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use vox_orchestrator::{
    CompletionAttestation, FileAffinity, Orchestrator, OrchestratorConfig, TaskPriority,
    types::{TaskDescriptor, TaskId},
};

fn e2e_completion_attestation() -> CompletionAttestation {
    CompletionAttestation {
        checks_passed: vec!["peer_review_approved".to_string()],
        ..Default::default()
    }
}

/// Whole-test wall timeout.
const E2E_TEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Per-phase await timeout (pinpoints stall before whole-test timeout).
const PHASE_TIMEOUT: Duration = Duration::from_secs(30);
/// Watchdog interval (~3s per user preference).
const WATCHDOG_INTERVAL: Duration = Duration::from_secs(3);

const MAX_DRAIN_INNER_ITERS_PER_AGENT: usize = 50_000;
const MAX_COMPLETIONS_PER_DRAIN_CALL: usize = 100_000;

fn forensic_log_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/e2e-logs")
}

fn wall_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// File-backed forensic logger for one test invocation.
#[derive(Clone)]
struct E2eForensic {
    test_name: String,
    log_path: PathBuf,
    file: Arc<Mutex<std::fs::File>>,
    watchdog_running: Arc<AtomicBool>,
}

impl E2eForensic {
    fn begin(test_name: &str) -> Self {
        let dir = forensic_log_dir();
        let _ = std::fs::create_dir_all(&dir);
        let pid = std::process::id();
        let safe: String = test_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let log_path = dir.join(format!("orchestrator_e2e_{safe}_{pid}.log"));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .unwrap_or_else(|e| panic!("open forensic log {}: {e}", log_path.display()));

        let slf = Self {
            test_name: test_name.to_string(),
            log_path,
            file: Arc::new(Mutex::new(file)),
            watchdog_running: Arc::new(AtomicBool::new(false)),
        };
        slf.log(&format!(
            "===== begin test={} pid={} log={} =====",
            test_name,
            pid,
            slf.log_path.display()
        ));
        slf
    }

    fn log_path(&self) -> &PathBuf {
        &self.log_path
    }

    fn log(&self, message: &str) {
        let tid = thread::current().id();
        let line = format!("[{}] {:?} {}\n", wall_ms(), tid, message);
        let mut f = self.file.lock().expect("forensic log mutex");
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
        eprintln!(
            "[orchestrator_e2e:{}] {:?} {}",
            self.test_name, tid, message
        );
    }

    fn log_status_dump(&self, orch: &Orchestrator, tag: &str) {
        let st = orch.status();
        self.log(&format!("status[{tag}] {:?}", st));
    }

    fn dump_assignments_and_traces(&self, orch: &Orchestrator, tag: &str) {
        let assigns = orch.task_assignments_copy();
        self.log(&format!(
            "dump[{tag}] task_assignments count={} {:?}",
            assigns.len(),
            assigns
        ));
        for (tid, aid) in &assigns {
            if let Some(steps) = orch.task_trace(*tid) {
                self.log(&format!(
                    "dump[{tag}] task_trace task={} agent={:?} steps={:?}",
                    tid.0, aid, steps
                ));
            }
        }
    }

    fn dump_full_state(&self, orch: &Orchestrator, reason: &str) {
        self.log(&format!("FULL_STATE reason={reason}"));
        self.log_status_dump(orch, reason);
        self.dump_assignments_and_traces(orch, reason);
    }

    /// Spawn a **native OS thread** that snapshots `status()` every `WATCHDOG_INTERVAL`.
    ///
    /// This is intentional: a Tokio-based watchdog does not run if every worker is blocked
    /// or deadlocked, which is exactly when you need evidence in the log file.
    fn start_watchdog(&self, orch: Arc<Orchestrator>) -> thread::JoinHandle<()> {
        self.watchdog_running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.watchdog_running);
        let fe = self.clone();
        thread::spawn(move || {
            let mut tick = 0u64;
            while running.load(Ordering::SeqCst) {
                // Chunk sleep so `stop_watchdog` + `join` return quickly when the test finishes
                // (a single `sleep(WATCHDOG_INTERVAL)` would block shutdown for the full interval).
                let mut waited = Duration::ZERO;
                while running.load(Ordering::SeqCst) && waited < WATCHDOG_INTERVAL {
                    thread::sleep(Duration::from_millis(100));
                    waited += Duration::from_millis(100);
                }
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                tick += 1;
                let st = orch.status();
                fe.log(&format!(
                    "watchdog[{tick}] enabled={} agents={} queued={} in_progress_count={} completed={} locked_files={} contention={}",
                    st.enabled,
                    st.agent_count,
                    st.total_queued,
                    st.total_in_progress,
                    st.total_completed,
                    st.locked_files,
                    st.total_contention
                ));
                for a in &st.agents {
                    fe.log(&format!(
                        "watchdog[{tick}] agent id={} name={} queued={} in_progress={} completed={} paused={}",
                        a.id.0,
                        a.name,
                        a.queued,
                        a.in_progress,
                        a.completed,
                        a.paused
                    ));
                }
            }
            fe.log("watchdog stopped (os thread)");
        })
    }

    fn stop_watchdog(&self, handle: thread::JoinHandle<()>) {
        self.watchdog_running.store(false, Ordering::SeqCst);
        let _ = handle.join();
    }
}

fn test_config() -> OrchestratorConfig {
    let mut config = OrchestratorConfig::for_testing();
    config.max_agents = 4;
    config
}

/// Isolated filesystem namespace for manifest paths so `capture_snapshot` never reads the real
/// workspace (Windows AV / odd CWD-relative resolution caused multi-minute stalls).
fn e2e_temp_root() -> tempfile::TempDir {
    tempfile::tempdir().expect("orchestrator e2e tempdir")
}

fn e2e_file(root: &Path, name: &str) -> PathBuf {
    root.join(name)
}

async fn run_with_timeout<F, T>(
    forensic: &E2eForensic,
    test_name: &str,
    duration: Duration,
    fut: F,
) -> T
where
    F: Future<Output = T>,
{
    let started = Instant::now();
    forensic.log(&format!(
        "run_with_timeout enter wall_timeout={duration:?} (outer); phase_timeout={PHASE_TIMEOUT:?}"
    ));
    tokio::time::timeout(duration, fut)
        .await
        .unwrap_or_else(|_| {
            forensic.log("OUTER_TIMEOUT: tokio::time::timeout elapsed (dump follows)");
            panic!(
                "{test_name}: outer timeout after {duration:?} elapsed {:?}; see {}",
                started.elapsed(),
                forensic.log_path().display()
            );
        })
}

async fn await_phase<F, T>(forensic: &E2eForensic, orch: &Orchestrator, label: &str, fut: F) -> T
where
    F: Future<Output = T>,
{
    forensic.log(&format!("phase ENTER {label}"));
    let out = tokio::time::timeout(PHASE_TIMEOUT, fut)
        .await
        .unwrap_or_else(|_| {
            forensic.log(&format!("PHASE_TIMEOUT: {label} (dump follows)"));
            forensic.dump_full_state(orch, &format!("phase_timeout:{label}"));
            panic!(
                "phase timeout {label} after {PHASE_TIMEOUT:?}; see {}",
                forensic.log_path().display()
            );
        });
    forensic.log(&format!("phase OK {label}"));
    out
}

async fn submit_task_traced(
    forensic: &E2eForensic,
    orch: &Orchestrator,
    phase: &str,
    description: impl Into<String>,
    file_manifest: Vec<FileAffinity>,
    priority: Option<TaskPriority>,
    session_id: Option<String>,
) -> TaskId {
    let d = description.into();
    forensic.log(&format!("submit_task request phase={phase} desc={d:?}"));
    let res = await_phase(
        forensic,
        orch,
        &format!("{phase}.submit_task"),
        orch.submit_task(d, file_manifest, priority, session_id, None),
    )
    .await;
    let tid = match res {
        Ok(id) => id,
        Err(e) => {
            forensic.dump_full_state(orch, &format!("submit_err:{phase}"));
            panic!("submit_task err {phase}: {e:?}");
        }
    };
    forensic.log(&format!("submit_task ok phase={phase} task_id={}", tid.0));
    tid
}

async fn submit_batch_traced(
    forensic: &E2eForensic,
    orch: &Orchestrator,
    phase: &str,
    descriptors: Vec<TaskDescriptor>,
) -> Vec<TaskId> {
    forensic.log(&format!(
        "submit_batch request phase={phase} len={}",
        descriptors.len()
    ));
    let res = await_phase(
        forensic,
        orch,
        &format!("{phase}.submit_batch"),
        orch.submit_batch(descriptors),
    )
    .await;
    let ids = match res {
        Ok(v) => v,
        Err(e) => {
            forensic.dump_full_state(orch, &format!("submit_batch_err:{phase}"));
            panic!("submit_batch err {phase}: {e:?}");
        }
    };
    forensic.log(&format!(
        "submit_batch ok phase={phase} ids={:?}",
        ids.iter().map(|t| t.0).collect::<Vec<_>>()
    ));
    ids
}

async fn complete_task_traced(
    forensic: &E2eForensic,
    orch: &Orchestrator,
    phase: &str,
    task_id: TaskId,
) {
    let res = await_phase(
        forensic,
        orch,
        &format!("{phase}.complete_task({})", task_id.0),
        orch.complete_task_with_attestation(task_id, Some(e2e_completion_attestation())),
    )
    .await;
    if let Err(e) = res {
        forensic.dump_full_state(orch, &format!("complete_err:{phase}:{}", task_id.0));
        panic!("complete_task err {}: {e:?}", task_id.0);
    }
    let trace = orch.task_trace(task_id);
    forensic.log(&format!(
        "complete_task ok phase={phase} task={} trace={:?}",
        task_id.0, trace
    ));
}

async fn fail_task_traced(
    forensic: &E2eForensic,
    orch: &Orchestrator,
    phase: &str,
    task_id: TaskId,
    reason: String,
) {
    let res = await_phase(
        forensic,
        orch,
        &format!("{phase}.fail_task({})", task_id.0),
        orch.fail_task(task_id, reason),
    )
    .await;
    if let Err(e) = res {
        forensic.dump_full_state(orch, &format!("fail_err:{phase}:{}", task_id.0));
        panic!("fail_task err {}: {e:?}", task_id.0);
    }
    forensic.log(&format!("fail_task ok phase={phase} task={}", task_id.0));
}

async fn drain_and_complete_all_bounded(
    forensic: &E2eForensic,
    orch: &Orchestrator,
    drain_label: &str,
) {
    let mut total_completions = 0usize;
    let ids = orch.agent_ids();
    forensic.log(&format!(
        "{drain_label}: drain begin, {} agent(s) ids={:?}",
        ids.len(),
        ids.iter().map(|a| a.0).collect::<Vec<_>>()
    ));

    for id in ids {
        let mut inner_rounds = 0usize;
        loop {
            inner_rounds += 1;
            assert!(
                inner_rounds <= MAX_DRAIN_INNER_ITERS_PER_AGENT,
                "drain: agent {id:?} inner bound; see {}",
                forensic.log_path().display()
            );

            let task_id = if let Some(queue_handle) = orch.get_agent_queue_mut(id) {
                if let Some(task) = queue_handle.write().unwrap().dequeue() {
                    Some(task.id)
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(tid) = task_id {
                forensic.log(&format!(
                    "{drain_label}: dequeued agent={} task={}",
                    id.0, tid.0
                ));
                complete_task_traced(forensic, orch, drain_label, tid).await;
                total_completions += 1;
                assert!(
                    total_completions <= MAX_COMPLETIONS_PER_DRAIN_CALL,
                    "drain: max completions; see {}",
                    forensic.log_path().display()
                );
            } else {
                break;
            }
        }
    }
    forensic.log(&format!(
        "{drain_label}: drain done, {total_completions} completion(s)"
    ));
}

async fn run_e2e_body_with<F, Fut>(
    test_name: &'static str,
    make_orch: impl FnOnce() -> Orchestrator,
    body: F,
) where
    F: FnOnce(Arc<E2eForensic>, Arc<Orchestrator>) -> Fut,
    Fut: Future<Output = ()>,
{
    let forensic = Arc::new(E2eForensic::begin(test_name));
    let orch = Arc::new(make_orch());
    forensic.log("Orchestrator::new ok (run_e2e_body_with)");

    let watch = forensic.start_watchdog(Arc::clone(&orch));
    run_with_timeout(
        forensic.as_ref(),
        test_name,
        E2E_TEST_TIMEOUT,
        body(Arc::clone(&forensic), Arc::clone(&orch)),
    )
    .await;

    forensic.stop_watchdog(watch);
}

async fn run_e2e_body<F, Fut>(test_name: &'static str, body: F)
where
    F: FnOnce(Arc<E2eForensic>, Arc<Orchestrator>) -> Fut,
    Fut: Future<Output = ()>,
{
    run_e2e_body_with(test_name, || Orchestrator::new(test_config()), body).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires running orchestrator stack with npm test infrastructure — owner: integration-tests sunset: 2026-12-31"]
async fn e2e_multi_agent_concurrent_edits() {
    run_e2e_body("e2e_multi_agent_concurrent_edits", |fe, orch| async move {
        let _tmp = e2e_temp_root();
        let root = _tmp.path();
        let _ = submit_task_traced(
            fe.as_ref(),
            orch.as_ref(),
            "t1",
            "Edit A",
            vec![FileAffinity::write(e2e_file(root, "a.rs"))],
            Some(TaskPriority::Normal),
            None,
        )
        .await;
        let _ = submit_task_traced(
            fe.as_ref(),
            orch.as_ref(),
            "t2",
            "Edit B",
            vec![FileAffinity::write(e2e_file(root, "b.rs"))],
            Some(TaskPriority::Normal),
            None,
        )
        .await;
        drain_and_complete_all_bounded(fe.as_ref(), orch.as_ref(), "drain1").await;
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires running orchestrator stack with npm test infrastructure — owner: integration-tests sunset: 2026-12-31"]
async fn e2e_task_queue_drain() {
    run_e2e_body("e2e_task_queue_drain", |fe, orch| async move {
        let _tmp = e2e_temp_root();
        let root = _tmp.path();
        for i in 0..10 {
            let _ = submit_task_traced(
                fe.as_ref(),
                orch.as_ref(),
                "e2e_task_queue_drain",
                format!("Task {i}"),
                vec![FileAffinity::write(e2e_file(root, "shared.rs"))],
                Some(TaskPriority::Normal),
                None,
            )
            .await;
        }
        let st = orch.status();
        assert_eq!(st.total_queued + st.total_in_progress, 10);
        drain_and_complete_all_bounded(fe.as_ref(), &orch, "drain1").await;
        let snap = orch.status();
        assert_eq!(snap.total_completed, 10);
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn e2e_context_sharing_across_agents() {
    run_e2e_body("e2e_context_sharing_across_agents", |fe, orch| async move {
        orch.context_store().write().unwrap().set(
            vox_orchestrator::types::AgentId(1),
            "shared_var",
            "secret_value",
            10,
        );
        fe.log("context_store set");
        let val = orch
            .context_store()
            .read()
            .unwrap()
            .get("shared_var")
            .expect("should exist")
            .clone();
        assert_eq!(val, "secret_value");
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn e2e_timeout_and_retry() {
    run_e2e_body_with(
        "e2e_timeout_and_retry",
        || {
            let mut cfg = test_config();
            cfg.lock_timeout_ms = 10;
            Orchestrator::new(cfg)
        },
        |fe, orch| async move {
            let _tmp = e2e_temp_root();
            let root = _tmp.path();
            let t1 = submit_task_traced(
                fe.as_ref(),
                orch.as_ref(),
                "fail_path",
                "Timeout Task",
                vec![FileAffinity::write(e2e_file(root, "c.rs"))],
                Some(TaskPriority::Normal),
                None,
            )
            .await;

            fe.log("manual dequeue for fail_path");
            if let Some(q_handle) = orch.get_agent_queue_mut(orch.agent_ids()[0]) {
                let _ = q_handle.write().unwrap().dequeue();
            }
            fail_task_traced(
                fe.as_ref(),
                orch.as_ref(),
                "fail_path",
                t1,
                "simulated failure".to_string(),
            )
            .await;
        },
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires running orchestrator stack with npm test infrastructure — owner: integration-tests sunset: 2026-12-31"]
async fn e2e_dependency_chain() {
    run_e2e_body("e2e_dependency_chain", |fe, orch| async move {
        let _tmp = e2e_temp_root();
        let root = _tmp.path();
        let _ = submit_task_traced(
            fe.as_ref(),
            orch.as_ref(),
            "dep",
            "Dep 1",
            vec![FileAffinity::write(e2e_file(root, "a.rs"))],
            Some(TaskPriority::Normal),
            None,
        )
        .await;
        drain_and_complete_all_bounded(fe.as_ref(), &orch, "drain1").await;
        let snap = orch.status();
        assert_eq!(snap.total_completed, 1);
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires running orchestrator stack with npm test infrastructure — owner: integration-tests sunset: 2026-12-31"]
async fn e2e_lock_contention_resolved() {
    run_e2e_body("e2e_lock_contention_resolved", |fe, orch| async move {
        let _tmp = e2e_temp_root();
        let root = _tmp.path();
        let locked = e2e_file(root, "locked.rs");
        let _ = submit_task_traced(
            fe.as_ref(),
            orch.as_ref(),
            "c1",
            "Contender 1",
            vec![FileAffinity::write(locked.clone())],
            Some(TaskPriority::Normal),
            None,
        )
        .await;
        let _ = submit_task_traced(
            fe.as_ref(),
            orch.as_ref(),
            "c2",
            "Contender 2",
            vec![FileAffinity::write(locked)],
            Some(TaskPriority::Normal),
            None,
        )
        .await;
        drain_and_complete_all_bounded(fe.as_ref(), &orch, "lock_pass1").await;
        drain_and_complete_all_bounded(fe.as_ref(), &orch, "lock_pass2").await;
        assert_eq!(orch.status().total_completed, 2);
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires running orchestrator stack with npm test infrastructure — owner: integration-tests sunset: 2026-12-31"]
async fn e2e_batch_submission() {
    run_e2e_body("e2e_batch_submission", |fe, orch| async move {
        let _tmp = e2e_temp_root();
        let root = _tmp.path();
        let batch = submit_batch_traced(
            fe.as_ref(),
            orch.as_ref(),
            "batch",
            vec![
                TaskDescriptor {
                    description: "Batch 1".to_string(),
                    priority: None,
                    file_manifest: vec![FileAffinity::write(e2e_file(root, "b1.rs"))],
                    depends_on: vec![],
                    temp_deps: vec![],
                    capability_requirements: None,
                    session_id: None,
                    thread_id: None,
                    requires_approval: false,
                    test_decision: None,
                    tenant_id: None,
                },
                TaskDescriptor {
                    description: "Batch 2".to_string(),
                    priority: None,
                    file_manifest: vec![FileAffinity::write(e2e_file(root, "b2.rs"))],
                    depends_on: vec![],
                    temp_deps: vec![],
                    capability_requirements: None,
                    session_id: None,
                    thread_id: None,
                    requires_approval: false,
                    test_decision: None,
                    tenant_id: None,
                },
            ],
        )
        .await;
        assert_eq!(batch.len(), 2);
        drain_and_complete_all_bounded(fe.as_ref(), &orch, "batch_drain").await;
        assert_eq!(orch.status().total_completed, 2);
    })
    .await;
}
