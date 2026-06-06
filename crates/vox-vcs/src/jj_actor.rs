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

use crate::backend::{VcsBackend, VcsError};
use crate::jj_backend::JjBackend;
use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tokio::sync::oneshot;

// ─── Command enum ──────────────────────────────────────────────────────────

type Reply<T> = oneshot::Sender<Result<T, VcsError>>;

/// One variant per [`VcsBackend`] method, plus `Shutdown`.
///
/// Each variant carries the method's arguments and a reply channel.  The enum
/// (and all argument types) are `Send`, enabling the channel to cross thread
/// boundaries.
pub enum Command {
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
        name: String,
        url: String,
        reply: Reply<()>,
    },
    Push {
        remote: String,
        bookmark: String,
        change: ChangeId,
        reply: Reply<()>,
    },
    Shutdown,
}

// SAFETY: All variant fields are Send (PathBuf, String, ChangeId, oneshot::Sender).
// The compiler will verify this automatically via the Send bound on mpsc::Sender.

// ─── JjActor ───────────────────────────────────────────────────────────────

/// Internal actor — not exposed publicly; callers use [`JjActorHandle`].
pub struct JjActor;

impl JjActor {
    /// Spawn the actor thread for the jj workspace at `root`.
    ///
    /// Blocks until the initial `JjBackend::open` completes (on the actor
    /// thread), then returns a [`JjActorHandle`] if the open succeeded, or
    /// `VcsError` if it failed.
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
                        let result = rt.block_on(engine.snapshot(label.as_deref(), paths));
                        let _ = reply.send(result);
                    }

                    Command::Changes { reply } => {
                        let result = rt.block_on(engine.changes());
                        let _ = reply.send(result);
                    }

                    Command::Diff { a, b, reply } => {
                        let result = rt.block_on(engine.diff(a, b));
                        let _ = reply.send(result);
                    }

                    Command::Undo { reply } => {
                        let result = rt.block_on(engine.undo());
                        let _ = reply.send(result);
                    }

                    Command::Conflicts { reply } => {
                        let result = rt.block_on(engine.conflicts());
                        let _ = reply.send(result);
                    }

                    Command::Resolve {
                        path,
                        strategy,
                        reply,
                    } => {
                        let result = rt.block_on(engine.resolve(&path, strategy));
                        let _ = reply.send(result);
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
                        let result = rt.block_on(engine.push(&remote, &bookmark, change));
                        let _ = reply.send(result);
                    }
                }
            }
            // engine drops here on the actor thread — correct for !Send types.
        });

        // Wait for the startup result (blocking; the open is fast).
        startup_rx
            .recv()
            .map_err(|_| VcsError::Unavailable("jj actor thread died before startup".into()))??;

        Ok(JjActorHandle { tx: cmd_tx })
    }
}

// ─── JjActorHandle ─────────────────────────────────────────────────────────

/// `Send + Sync` handle to the jj actor thread.
///
/// Clone freely; the actor lives as long as at least one handle exists.
#[derive(Clone)]
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
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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

    /// A dead actor must return `Err(Unavailable)`, never hang.
    #[tokio::test(flavor = "multi_thread")]
    async fn actor_dead_thread_returns_err() {
        let dir = tempfile::tempdir().unwrap();
        let handle = JjActor::spawn(dir.path().to_path_buf()).expect("spawn actor");

        // Kill the actor by sending Shutdown directly via the raw sender.
        handle.tx.send(Command::Shutdown).expect("send shutdown");

        // Give the actor thread a moment to process the shutdown.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Now the actor channel is closed; any call must return Err, not hang.
        let result = tokio::time::timeout(Duration::from_secs(5), handle.changes())
            .await
            .expect("must not hang (timeout)");

        assert!(
            matches!(result, Err(VcsError::Unavailable(_))),
            "dead actor must return Unavailable, got: {:?}",
            result
        );
    }
}
