//! Runtime backend selection for Oratio STT.
//!
//! Priority: `VOX_ORATIO_BACKEND` env → feature flags → Candle Whisper fallback.

use crate::backends::asr_backend::AsrBackend;

#[cfg(feature = "stt-candle")]
use crate::backends::candle_whisper::CandleWhisperBackend;

/// Test-only instrumentation: counts invocations of `create_backend()`'s body,
/// used to assert that `with_cached_backend` constructs the backend once.
#[cfg(test)]
pub(crate) static CREATE_BACKEND_CALL_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Instantiate the configured STT backend.
///
/// # Env
/// - `VOX_ORATIO_BACKEND=auto` (default) — picks Sherpa if compiled in, else Candle
/// - `VOX_ORATIO_BACKEND=whisper` — always Candle Whisper
/// - `VOX_ORATIO_BACKEND=sherpa` — always Sherpa (returns error if feature not compiled)
pub fn create_backend() -> anyhow::Result<Box<dyn AsrBackend>> {
    #[cfg(test)]
    {
        CREATE_BACKEND_CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if std::env::var("VOX_TEST_FORCE_BACKEND_FAIL").is_ok() {
            anyhow::bail!("forced failure for test (VOX_TEST_FORCE_BACKEND_FAIL set)");
        }
    }

    let backend_env = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioBackend)
        .expose()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "auto".to_string());
    let backend_env = backend_env.trim().to_ascii_lowercase();

    match backend_env.as_str() {
        "auto" | "" => {
            #[cfg(feature = "stt-sherpa")]
            {
                match crate::backends::sherpa_onnx::SherpaOnnxBackend::new() {
                    Ok(backend) => Ok(Box::new(backend) as Box<dyn AsrBackend>),
                    Err(e) => {
                        tracing::warn!(
                            target: "vox_oratio_backend",
                            event = "sherpa_init_failed_falling_back",
                            error = %e,
                            "Sherpa-ONNX (Parakeet) init failed; falling back to Candle Whisper"
                        );
                        #[cfg(feature = "stt-candle")]
                        {
                            Ok(Box::new(CandleWhisperBackend) as Box<dyn AsrBackend>)
                        }
                        #[cfg(not(feature = "stt-candle"))]
                        {
                            Err(e)
                        }
                    }
                }
            }
            #[cfg(all(feature = "stt-candle", not(feature = "stt-sherpa")))]
            {
                Ok(Box::new(CandleWhisperBackend))
            }
            #[cfg(not(any(feature = "stt-candle", feature = "stt-sherpa")))]
            anyhow::bail!(
                "No STT backend compiled in. Enable `stt-candle` or `stt-sherpa` feature."
            );
        }
        "whisper" | "candle" => {
            #[cfg(feature = "stt-candle")]
            return Ok(Box::new(CandleWhisperBackend));
            #[cfg(not(feature = "stt-candle"))]
            anyhow::bail!("Backend 'whisper' selected but `stt-candle` feature not compiled.");
        }
        "sherpa" => {
            #[cfg(feature = "stt-sherpa")]
            return Ok(Box::new(
                crate::backends::sherpa_onnx::SherpaOnnxBackend::new()?,
            ));
            #[cfg(not(feature = "stt-sherpa"))]
            anyhow::bail!("Backend 'sherpa' selected but `stt-sherpa` feature not compiled.");
        }
        other => anyhow::bail!("Unknown VOX_ORATIO_BACKEND value: {other:?}"),
    }
}

/// Process-lifetime cache for the ASR backend instance.
///
/// `create_backend()` can be expensive (model resolution, ONNX Runtime
/// session construction) — for Sherpa-ONNX/Parakeet this includes loading a
/// ~671MB model. Without caching, calling `create_backend()` per utterance
/// (as the file-transcription path in `traits.rs` does) would pay that full
/// cost on every dictation stop. This cache constructs the backend once and
/// reuses it for the lifetime of the process.
///
/// A construction failure leaves the slot `None` so the *next* call retries
/// `create_backend()` from scratch, instead of latching into a permanent
/// "no backend" state until restart.
///
/// Stored as `Arc`, not `Box`: `with_cached_backend` only needs the mutex
/// held long enough to construct-once-and-clone the handle, not for the
/// duration of the (potentially slow) inference call `f` makes with it —
/// holding a `MutexGuard` across `f` would serialize every concurrent
/// transcription in the process on ASR inference time, not just backend
/// construction, which nothing about this cache is meant to require.
static BACKEND: std::sync::Mutex<Option<std::sync::Arc<dyn AsrBackend>>> =
    std::sync::Mutex::new(None);

/// Run `f` against the cached ASR backend, constructing it on first
/// successful call. Safe to call from multiple threads: the backend is
/// constructed at most once (barring a failed attempt, which is retried on
/// the next call). The lock is released before `f` runs, so concurrent
/// transcriptions don't serialize on each other — only on the one-time
/// construction.
pub fn with_cached_backend<F, R>(f: F) -> anyhow::Result<R>
where
    F: FnOnce(&dyn AsrBackend) -> anyhow::Result<R>,
{
    let backend = {
        let mut guard = BACKEND.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if guard.is_none() {
            *guard = Some(std::sync::Arc::from(create_backend()?));
        }
        guard.as_ref().expect("just populated").clone()
    };
    f(backend.as_ref())
}

/// Test-only: clears the cache slot so each test starts from a known state.
/// `BACKEND` is a process-wide static shared by every test in this binary;
/// without this, whichever test runs first "wins" the cache for the rest.
#[cfg(test)]
fn reset_cache_for_test() {
    let mut guard = BACKEND.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    /// Serializes tests in this module: they share the process-wide `BACKEND`
    /// cache and mutate `VOX_ORATIO_BACKEND` / `VOX_TEST_FORCE_BACKEND_FAIL`
    /// env vars, so they cannot run concurrently with each other.
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn with_cached_backend_constructs_once() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        reset_cache_for_test();
        // SAFETY: test-only env mutation, serialized by TEST_LOCK.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_TEST_FORCE_BACKEND_FAIL");
            std::env::set_var("VOX_ORATIO_BACKEND", "whisper");
        }

        let before = CREATE_BACKEND_CALL_COUNT.load(Ordering::SeqCst);
        with_cached_backend(|_backend| Ok(())).expect("first call should construct backend");
        with_cached_backend(|_backend| Ok(())).expect("second call should reuse cached backend");
        let after = CREATE_BACKEND_CALL_COUNT.load(Ordering::SeqCst);

        // SAFETY: test-only env mutation, serialized by TEST_LOCK.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_BACKEND");
        }

        assert_eq!(
            after - before,
            1,
            "create_backend() should run exactly once across two with_cached_backend calls"
        );
    }

    #[test]
    fn with_cached_backend_retries_after_failure() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        reset_cache_for_test();
        // SAFETY: test-only env mutation, serialized by TEST_LOCK.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("VOX_ORATIO_BACKEND", "whisper");
            std::env::set_var("VOX_TEST_FORCE_BACKEND_FAIL", "1");
        }

        let first = with_cached_backend(|_backend| Ok(()));
        assert!(first.is_err(), "forced failure should propagate as Err");

        // "Fix" the condition that caused the failure.
        // SAFETY: test-only env mutation, serialized by TEST_LOCK.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_TEST_FORCE_BACKEND_FAIL");
        }

        let second = with_cached_backend(|_backend| Ok(()));

        // SAFETY: test-only env mutation, serialized by TEST_LOCK.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_BACKEND");
        }

        assert!(
            second.is_ok(),
            "a failed construction must not permanently latch the cache into a failure \
             state; the next call should retry create_backend() from scratch: {:?}",
            second.err()
        );
    }

    /// Regression test for the bug the `Box` -> `Arc` cache conversion fixed:
    /// `with_cached_backend` must release its lock before running `f`, so two
    /// concurrent calls don't serialize on each other's (potentially slow)
    /// inference. Proven via a deadlock construction: both threads' closures
    /// must reach a 2-party barrier to complete. If the lock were still held
    /// across `f` (the old `Box`-based bug), the second thread could not even
    /// *enter* its closure until the first thread's closure returns — but the
    /// first thread's closure is itself blocked on the barrier waiting for the
    /// second thread to arrive. That's a genuine deadlock, not just slowness,
    /// so a regression here fails by hanging rather than by a flaky timing
    /// assumption. Bounded by a channel + `recv_timeout` so CI fails fast
    /// (as a normal test failure) instead of hanging indefinitely.
    #[test]
    fn with_cached_backend_does_not_serialize_concurrent_calls() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        reset_cache_for_test();
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_TEST_FORCE_BACKEND_FAIL");
            std::env::set_var("VOX_ORATIO_BACKEND", "whisper");
        }
        // Warm the cache first so both threads below hit the "already
        // constructed" fast path, isolating the property under test (lock
        // scope around `f`) from construction itself.
        with_cached_backend(|_backend| Ok(())).expect("warm the cache");

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let (tx, rx) = std::sync::mpsc::channel();
        for _ in 0..2 {
            let barrier = barrier.clone();
            let tx = tx.clone();
            std::thread::spawn(move || {
                let result = with_cached_backend(|_backend| {
                    barrier.wait();
                    Ok(())
                });
                let _ = tx.send(result.is_ok());
            });
        }

        for _ in 0..2 {
            let ok = rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect(
                    "both with_cached_backend calls should complete within 5s; a hang here \
                     means the lock is being held across `f` again (the bug the Arc \
                     conversion fixed), deadlocking both threads on the shared barrier",
                );
            assert!(ok, "with_cached_backend call failed unexpectedly");
        }

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_BACKEND");
        }
    }

    #[test]
    fn create_backend_auto_falls_back_to_candle_when_sherpa_init_fails() {
        // Held for the duration of the test: see `crate::env_test_lock` (Task
        // 4 Step 1) — this env var is also mutated by sherpa_model_config's test.
        let _sherpa_env_guard = crate::env_test_lock::SHERPA_MODEL_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());

        let dir = tempfile::tempdir().expect("tempdir");
        // Deliberately empty: no real ONNX model files, so Sherpa-ONNX init
        // fails fast and offline. VOX_ORATIO_SHERPA_MODEL_DIR being set means
        // `resolve_sherpa_transducer_model_paths` short-circuits before the
        // HF Hub network branch entirely (see Task 4) — no network I/O here.
        // SAFETY: test-only env mutation, serialized by the locks above.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_BACKEND");
            std::env::set_var("VOX_ORATIO_SHERPA_MODEL_DIR", dir.path());
        }
        let result = create_backend();
        // SAFETY: test-only env mutation, serialized by the locks above.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_SHERPA_MODEL_DIR");
        }
        assert!(
            result.is_ok(),
            "auto mode must fall back to Candle Whisper when Sherpa-ONNX init \
             fails (empty model dir), not propagate the Sherpa error: {:?}",
            result.err()
        );
    }
}
