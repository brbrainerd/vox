//! The jj-actor: a dedicated OS thread that owns the `!Send` [`JjBackend`] and
//! exposes a **`Send + Sync`** async handle ([`JjActorHandle`]) that satisfies
//! `#[async_trait] VcsBackend`.
//!
//! ## Why an actor?
//!
//! jj-lib 0.42's async futures are `!Send` (interior `RefCell`/`OnceCell`
//! scattered across `Transaction`, `MutableRepo`, `dyn LockedWorkingCopy`, etc.).
//! A `Send`-future trait (`#[async_trait] VcsBackend`) therefore cannot be
//! implemented directly by `JjBackend`. The actor pattern solves this without
//! changing jj-lib:
//!
//! * The actor thread owns a **current-thread** tokio runtime; the `!Send`
//!   futures only ever run on that single thread.
//! * The handle queues `Command` messages (all `Send`) over a `std::sync::mpsc`
//!   channel and awaits a `tokio::sync::oneshot` reply — both of which are `Send`.
//! * From the orchestrator's perspective every VCS call is a normal `async fn`
//!   that can be `tokio::spawn`ed freely.
//!
//! ## Lifecycle
//!
//! When the last [`JjActorHandle`] clone is dropped the `Sender` closes, the
//! actor's `recv()` returns `Err`, and the loop exits — the thread and engine are
//! cleaned up by Rust's normal `Drop` chain.  The `JoinHandle` is detached (not
//! held) so nothing blocks in `Drop`.
//!
//! ## Panic containment
//!
//! Every `block_on` call is wrapped in `std::panic::catch_unwind`. A panicking
//! jj-lib operation returns `Err(VcsError::Unavailable)` to the caller but does
//! **not** terminate the actor loop. Note: the actor's `JjBackend` state may be
//! degraded after a mid-operation panic; a higher layer can re-spawn the actor
//! (via [`JjActor::spawn`]) if needed.

use crate::backend::{VcsBackend, VcsError};
use crate::jj_backend::JjBackend;
use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
use async_trait::async_trait;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::oneshot;
use vox_config::timeouts::{D_5S, D_10S, D_30S, D_250MS};

/// Upper bound on a single jj operation inside the actor. Remote push/fetch shell
/// out to `git` and can legitimately take a while, so this is deliberately
/// generous: its purpose is to stop a *wedged* operation from blocking the actor
/// thread — and every command queued behind it — forever, not to bound normal
/// latency. On elapse the operation's future is dropped (releasing its jj locks /
/// mutex guard) and the caller gets `VcsError::Unavailable`.
const OP_TIMEOUT: Duration = Duration::from_secs(120);

/// Run a single jj operation future under a timeout, mapping an elapsed deadline
/// to a `VcsError` instead of hanging the actor. Kept as a free async fn (rather
/// than inlined into the `guarded!` macro) so the timeout logic is unit-testable
/// without a real wedged jj operation.
async fn with_op_timeout<T>(
    timeout: Duration,
    fut: impl Future<Output = Result<T, VcsError>>,
) -> Result<T, VcsError> {
    match tokio::time::timeout(timeout, fut).await {
        Ok(result) => result,
        Err(_elapsed) => Err(VcsError::Unavailable(format!(
            "jj operation timed out after {}s",
            timeout.as_secs()
        ))),
    }
}

// ─── Command enum ──────────────────────────────────────────────────────────

type Reply<T> = oneshot::Sender<Result<T, VcsError>>;

/// One variant per [`VcsBackend`] method, plus `Shutdown`.
///
/// Each variant carries the method's arguments and a reply channel.  The enum
/// (and all argument types) are `Send`, enabling the channel to cross thread
/// boundaries.
///
/// `Command` is an internal wire protocol; it is `pub(crate)` only.
pub(crate) enum Command {
    Snapshot {
        label: Option<String>,
        paths: Vec<PathBuf>,
        reply: Reply<ChangeId>,
    },
    Changes {
        reply: Reply<Vec<Change>>,
    },
    Diff {
        a: Option<ChangeId>,
        b: Option<ChangeId>,
        reply: Reply<Diff>,
    },
    Undo {
        reply: Reply<ChangeId>,
    },
    Conflicts {
        reply: Reply<Vec<Conflict>>,
    },
    Resolve {
        path: PathBuf,
        strategy: ResolveStrategy,
        reply: Reply<()>,
    },
    AddRemote {
        // Carried for API symmetry; the actor's AddRemote handler is a
        // deliberate no-op stub (registers remotes out-of-band) so these
        // are never read. See the handler note below.
        #[allow(dead_code)]
        name: String,
        #[allow(dead_code)]
        url: String,
        reply: Reply<()>,
    },
    Push {
        remote: String,
        bookmark: String,
        change: ChangeId,
        reply: Reply<()>,
    },
    CreateBranch {
        name: String,
        reply: Reply<()>,
    },
    Shutdown,
    /// Test-only variant: forces a panic inside the catch_unwind wrapper to
    /// verify the actor survives a panicking operation.
    #[cfg(test)]
    TestPanic {
        reply: Reply<()>,
    },
}

// SAFETY: All variant fields are Send (PathBuf, String, ChangeId, oneshot::Sender).
// The compiler will verify this automatically via the Send bound on mpsc::Sender.

// ─── JjActor ───────────────────────────────────────────────────────────────

/// Spawn the jj actor thread for the workspace at `root`, returning a
/// `Send + Sync` [`JjActorHandle`].
///
/// Blocks the calling thread until the jj workspace opens (or 30 s timeout);
/// from async contexts call inside `tokio::task::spawn_blocking`. Returns
/// `VcsError` if the open failed or timed out.
pub fn spawn_handle(root: PathBuf) -> Result<JjActorHandle, VcsError> {
    JjActor::spawn(root)
}

/// Internal actor — not exposed publicly; callers use [`JjActorHandle`].
pub(crate) struct JjActor;

impl JjActor {
    /// Spawn the actor thread for the jj workspace at `root`.
    ///
    /// Blocks the calling thread until the jj workspace opens (or 30 s timeout).
    /// From async contexts, call inside `tokio::task::spawn_blocking`.
    ///
    /// Returns a [`JjActorHandle`] if the open succeeded, or `VcsError` if it
    /// failed or timed out.
    ///
    /// The actor thread is detached — its lifetime is tied to the channel: when
    /// all [`JjActorHandle`] clones drop, the sender closes, `recv()` errors,
    /// and the thread terminates naturally.
    pub fn spawn(root: PathBuf) -> Result<JjActorHandle, VcsError> {
        // Startup synchronization: the actor sends back the open result before
        // entering its command loop.
        let (startup_tx, startup_rx) = std::sync::mpsc::channel::<Result<(), VcsError>>();
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();

        std::thread::spawn(move || {
            // Build a current-thread tokio runtime on this dedicated thread.
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("jj actor: failed to build tokio runtime");

            // Open the jj engine.  This is safe: we are NOT inside a tokio
            // worker thread here, so there is no nested-runtime panic.
            let mut engine = match rt.block_on(JjBackend::open(&root)) {
                Ok(e) => {
                    let _ = startup_tx.send(Ok(()));
                    e
                }
                Err(e) => {
                    let _ = startup_tx.send(Err(e));
                    return; // thread exits; cmd_rx drops, JjActorHandle will error.
                }
            };

            /// Run `$fut` inside `catch_unwind`; on panic send `Unavailable`
            /// on `$reply` and `continue` the loop so the actor stays alive.
            macro_rules! guarded {
                ($rt:expr, $fut:expr, $reply:expr) => {{
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        $rt.block_on(with_op_timeout(OP_TIMEOUT, $fut))
                    }));
                    match result {
                        Ok(method_result) => {
                            let _ = $reply.send(method_result);
                        }
                        Err(_panic) => {
                            eprintln!("jj actor: operation panicked");
                            let _ = $reply
                                .send(Err(VcsError::Unavailable("jj operation panicked".into())));
                        }
                    }
                }};
            }

            // Command loop.
            loop {
                let cmd = match cmd_rx.recv() {
                    Ok(c) => c,
                    Err(_) => break, // All senders dropped → clean shutdown.
                };

                match cmd {
                    Command::Shutdown => break,

                    Command::Snapshot {
                        label,
                        paths,
                        reply,
                    } => {
                        guarded!(rt, engine.snapshot(label.as_deref(), paths), reply);
                    }

                    Command::Changes { reply } => {
                        guarded!(rt, engine.changes(), reply);
                    }

                    Command::Diff { a, b, reply } => {
                        guarded!(rt, engine.diff(a, b), reply);
                    }

                    Command::Undo { reply } => {
                        guarded!(rt, engine.undo(), reply);
                    }

                    Command::Conflicts { reply } => {
                        guarded!(rt, engine.conflicts(), reply);
                    }

                    Command::Resolve {
                        path,
                        strategy,
                        reply,
                    } => {
                        guarded!(rt, engine.resolve(&path, strategy), reply);
                    }

                    Command::AddRemote {
                        name: _,
                        url: _,
                        reply,
                    } => {
                        // The jj git add-remote API forces a gix dep; we keep
                        // gix out of vox-vcs (see jj_backend.rs Task 5 note).
                        let _ = reply.send(Err(VcsError::Unavailable(
                            "jj add_remote: register remote out-of-band via `git remote add`"
                                .into(),
                        )));
                    }

                    Command::Push {
                        remote,
                        bookmark,
                        change,
                        reply,
                    } => {
                        guarded!(rt, engine.push(&remote, &bookmark, change), reply);
                    }

                    Command::CreateBranch { name, reply } => {
                        guarded!(rt, engine.create_branch(&name), reply);
                    }

                    #[cfg(test)]
                    Command::TestPanic { reply } => {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            || -> Result<(), VcsError> {
                                panic!("test panic");
                            },
                        ));
                        match result {
                            Ok(method_result) => {
                                let _ = reply.send(method_result);
                            }
                            Err(_panic) => {
                                eprintln!("jj actor: operation panicked");
                                let _ = reply.send(Err(VcsError::Unavailable(
                                    "jj operation panicked".into(),
                                )));
                            }
                        }
                    }
                }
            }
            // engine drops here on the actor thread — correct for !Send types.
        });

        // Wait for the startup result (up to 30 s).
        match startup_rx.recv_timeout(D_30S) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(VcsError::Unavailable("jj actor open failed".into()));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                return Err(VcsError::Unavailable("jj actor open timed out".into()));
            }
        }

        Ok(JjActorHandle { tx: cmd_tx })
    }
}

// ─── JjActorHandle ─────────────────────────────────────────────────────────

/// `Send + Sync` handle to the jj actor thread.
///
/// Clone freely; the actor lives as long as at least one handle exists.
///
/// All methods serialize through the single actor thread; even logically
/// read-only calls (`changes`/`diff`) execute one-at-a-time, never
/// concurrently (jj-lib is single-threaded).
#[derive(Clone, Debug)]
pub struct JjActorHandle {
    tx: mpsc::Sender<Command>,
}

impl JjActorHandle {
    /// Send a command and await the reply, translating channel errors to
    /// `VcsError::Unavailable`.
    async fn call<T: Send + 'static>(
        &self,
        cmd_fn: impl FnOnce(Reply<T>) -> Command,
    ) -> Result<T, VcsError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(cmd_fn(reply_tx))
            .map_err(|_| VcsError::Unavailable("jj actor stopped".into()))?;
        reply_rx
            .await
            .map_err(|_| VcsError::Unavailable("jj actor thread died".into()))?
    }

    /// Request a clean shutdown of the actor thread.
    ///
    /// Returns `Err(Unavailable)` if the actor is already gone.
    pub fn shutdown(&self) -> Result<(), VcsError> {
        self.tx
            .send(Command::Shutdown)
            .map_err(|_| VcsError::Unavailable("jj actor already stopped".into()))
    }
}

// SAFETY: mpsc::Sender<Command> is Send (Command: Send), and Sync follows from
// the fact that send() takes &self and the channel is internally synchronized.
// The derive(Clone) produces the same tx clone which is also Send.
// Rust will enforce this automatically — this comment is for human readers.

#[async_trait]
impl VcsBackend for JjActorHandle {
    async fn snapshot(
        &mut self,
        label: Option<&str>,
        paths: Vec<PathBuf>,
    ) -> Result<ChangeId, VcsError> {
        let label = label.map(str::to_owned);
        self.call(|reply| Command::Snapshot {
            label,
            paths,
            reply,
        })
        .await
    }

    async fn changes(&self) -> Result<Vec<Change>, VcsError> {
        self.call(|reply| Command::Changes { reply }).await
    }

    async fn diff(&self, a: Option<ChangeId>, b: Option<ChangeId>) -> Result<Diff, VcsError> {
        self.call(|reply| Command::Diff { a, b, reply }).await
    }

    async fn undo(&mut self) -> Result<ChangeId, VcsError> {
        self.call(|reply| Command::Undo { reply }).await
    }

    async fn conflicts(&self) -> Result<Vec<Conflict>, VcsError> {
        self.call(|reply| Command::Conflicts { reply }).await
    }

    async fn resolve(&mut self, path: &Path, strategy: ResolveStrategy) -> Result<(), VcsError> {
        let path = path.to_path_buf();
        self.call(|reply| Command::Resolve {
            path,
            strategy,
            reply,
        })
        .await
    }

    async fn add_remote(&mut self, name: &str, url: &str) -> Result<(), VcsError> {
        let name = name.to_owned();
        let url = url.to_owned();
        self.call(|reply| Command::AddRemote { name, url, reply })
            .await
    }

    async fn push(
        &mut self,
        remote: &str,
        bookmark: &str,
        change: ChangeId,
    ) -> Result<(), VcsError> {
        let remote = remote.to_owned();
        let bookmark = bookmark.to_owned();
        self.call(|reply| Command::Push {
            remote,
            bookmark,
            change,
            reply,
        })
        .await
    }

    async fn create_branch(&mut self, name: &str) -> Result<(), VcsError> {
        let name = name.to_owned();
        self.call(|reply| Command::CreateBranch { name, reply })
            .await
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use vox_config::timeouts::{D_5S, D_10S, D_250MS};

    /// A wedged operation elapses to a `VcsError` (so the actor thread is freed
    /// for the next queued command) while a fast operation passes through cleanly.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn op_timeout_bounds_a_wedged_operation() {
        let slow = async {
            tokio::time::sleep(D_250MS).await;
            Ok::<(), VcsError>(())
        };
        let timed_out = with_op_timeout(Duration::from_millis(20), slow).await;
        assert!(
            matches!(timed_out, Err(VcsError::Unavailable(ref m)) if m.contains("timed out")),
            "a wedged operation must elapse to VcsError::Unavailable, got {timed_out:?}"
        );

        let fast = async { Ok::<i32, VcsError>(7) };
        let passed = with_op_timeout(D_5S, fast).await;
        assert_eq!(passed.unwrap(), 7, "a fast operation must pass through");
    }

    /// Basic actor functionality: snapshot + changes from within a real
    /// multi-thread tokio runtime.  Proves the actor works and the snapshot id
    /// appears in the change log.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn actor_snapshot_and_changes_from_runtime() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hello.txt"), b"hi").unwrap();

        let mut handle = JjActor::spawn(dir.path().to_path_buf()).expect("spawn actor");
        let id = handle
            .snapshot(Some("first"), vec![PathBuf::from("hello.txt")])
            .await
            .expect("snapshot");
        let changes = handle.changes().await.expect("changes");
        assert!(
            changes.iter().any(|c| c.id == id),
            "snapshot id must appear in the change log"
        );
    }

    /// P4 Task 5: create_branch over the actor handle succeeds end-to-end.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn actor_create_branch_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("br.txt"), b"branch").unwrap();
        let mut handle = JjActor::spawn(dir.path().to_path_buf()).expect("spawn actor");
        handle
            .snapshot(Some("base"), vec![PathBuf::from("br.txt")])
            .await
            .expect("snapshot");
        handle
            .create_branch("agent/3")
            .await
            .expect("create_branch over actor must succeed");
    }

    /// THE key test: proves `JjActorHandle` futures are `Send` and can cross
    /// `tokio::spawn`.  If `JjActorHandle` were `!Send` (or its futures were
    /// `!Send`), this test would FAIL TO COMPILE — exactly what the `?Send`
    /// trait could not do.
    #[tokio::test(flavor = "multi_thread")]
    async fn actor_call_survives_tokio_spawn() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("s.txt"), b"spawn").unwrap();

        let mut h = JjActor::spawn(dir.path().to_path_buf()).expect("spawn actor");
        // tokio::spawn requires the future to be Send.
        let jh = tokio::spawn(async move {
            h.snapshot(Some("spawned"), vec![PathBuf::from("s.txt")])
                .await
        });
        jh.await.unwrap().expect("snapshot inside tokio::spawn");
    }

    /// Panic containment: a panicking jj operation must return `Err(Unavailable)`
    /// to the caller AND the actor must remain alive to serve subsequent calls.
    #[tokio::test(flavor = "multi_thread")]
    async fn actor_survives_panicking_operation() {
        tokio::time::timeout(D_10S, async {
            let dir = tempfile::tempdir().unwrap();
            let handle = JjActor::spawn(dir.path().to_path_buf()).expect("spawn actor");

            // Send the test-panic command; actor must return Err, not crash.
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            handle
                .tx
                .send(Command::TestPanic { reply: reply_tx })
                .expect("send TestPanic");
            let panic_result = reply_rx.await.expect("reply received");
            assert!(
                matches!(panic_result, Err(VcsError::Unavailable(_))),
                "panicking op must return Unavailable, got: {:?}",
                panic_result
            );

            // The actor must still be alive: a subsequent normal call succeeds.
            let changes_result = handle.changes().await;
            assert!(
                changes_result.is_ok(),
                "actor must survive the panic and serve subsequent calls; got: {:?}",
                changes_result
            );
        })
        .await
        .expect("test must not hang");
    }

    /// A dead actor must return `Err(Unavailable)`, never hang.
    #[tokio::test(flavor = "multi_thread")]
    async fn actor_dead_thread_returns_err() {
        tokio::time::timeout(D_10S, async {
            let dir = tempfile::tempdir().unwrap();
            let handle = JjActor::spawn(dir.path().to_path_buf()).expect("spawn actor");

            // Shut the actor down cleanly via the public API.
            handle.shutdown().expect("shutdown");

            // Poll deterministically until the actor is gone (no fixed sleep).
            let mut got_err = false;
            for _ in 0..200 {
                if handle.changes().await.is_err() {
                    got_err = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(got_err, "calls must return Err once the actor has stopped");
        })
        .await
        .expect("test must not hang");
    }
}
