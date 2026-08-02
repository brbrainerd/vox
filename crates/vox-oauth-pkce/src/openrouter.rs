//! OpenRouter-specific PKCE loopback flow driver (RFC 8252 §7.3 pattern).

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::pkce::{self, PkcePair};

const OPENROUTER_AUTH_URL: &str = "https://openrouter.ai/auth";
const OPENROUTER_TOKEN_EXCHANGE_URL: &str = "https://openrouter.ai/api/v1/auth/keys";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("failed to bind loopback listener: {0}")]
    Bind(std::io::Error),
    #[error("failed to open system browser for {url}: {source}")]
    BrowserOpen {
        url: String,
        #[source]
        source: std::io::Error,
    },
    #[error("timed out waiting for OAuth callback ({0:?})")]
    TimedOut(Duration),
    #[error("callback state mismatch (possible CSRF)")]
    StateMismatch,
    #[error("token exchange failed: {0}")]
    TokenExchange(String),
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

/// Request body for the OpenRouter PKCE token-exchange call.
///
/// `Debug` is hand-written to **redact `code_verifier`**, following the same
/// discipline as `PkcePair` (this crate) and `EgressRequest`
/// (`vox_llm_egress`): the code verifier is a bearer-adjacent OAuth secret
/// and must never leak into logs/traces via e.g. `tracing::debug!(?req)`,
/// even though nothing in this file currently logs it.
#[derive(serde::Serialize)]
struct ExchangeRequest<'a> {
    code: &'a str,
    code_verifier: &'a str,
    code_challenge_method: &'a str,
}

impl std::fmt::Debug for ExchangeRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExchangeRequest")
            .field("code", &"[redacted]")
            .field(
                "code_verifier",
                &format!("[redacted len={}]", self.code_verifier.len()),
            )
            .field("code_challenge_method", &self.code_challenge_method)
            .finish()
    }
}

#[derive(Debug, Deserialize)]
struct ExchangeResponse {
    key: String,
}

struct CallbackState {
    expected_state: String,
    tx: std::sync::Mutex<Option<oneshot::Sender<Result<String, OAuthError>>>>,
    shutdown_tx: std::sync::Mutex<Option<oneshot::Sender<()>>>,
}

async fn callback_handler(
    State(state): State<Arc<CallbackState>>,
    Query(q): Query<CallbackQuery>,
) -> Html<&'static str> {
    // OpenRouter's documented OAuth contract does not mention a `state`
    // parameter being echoed back on the callback (verified against their
    // live docs during the second audit round) — reject only on an explicit
    // MISMATCH (state present but wrong), never on absence, or every real
    // login would fail a check OpenRouter never promised to honor. The PKCE
    // code_verifier check at token-exchange time is the real security
    // boundary either way. If empirical testing later shows OpenRouter DOES
    // echo `state`, tighten this back to required-and-matching.
    let result = match q.code {
        None => Err(OAuthError::TokenExchange("missing code in callback".into())),
        Some(code) => match q.state {
            Some(got_state) if got_state != state.expected_state => Err(OAuthError::StateMismatch),
            _ => Ok(code),
        },
    };
    // These mutexes guard nothing but `Option<oneshot::Sender<_>>` — a panic
    // while holding either lock can only happen inside `.take()` itself
    // (infallible), so poisoning here would only ever be caused by an
    // unrelated panic elsewhere while the lock happened to be held for the
    // instant of the `.take()`. Recovering the guard via `into_inner()` is
    // safe and strictly better than panicking this handler (network-
    // reachable, once per inbound HTTP request) over state that is still
    // perfectly usable — the alternative (`.expect()`) would crash the whole
    // loopback flow, including the one legitimate in-flight callback, over
    // an unrelated poison.
    if let Some(tx) = state
        .tx
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
    {
        let _ = tx.send(result);
    }
    // Signal graceful shutdown only now, after the response above has been
    // constructed — axum flushes it to the client before the connection
    // closes. This intentionally avoids a raw task abort(): aborting
    // immediately after receiving the callback is a documented failure mode
    // for this exact "one-shot loopback server" pattern, where the browser
    // can see a connection-reset instead of the success page.
    if let Some(shutdown_tx) = state
        .shutdown_tx
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
    {
        let _ = shutdown_tx.send(());
    }
    Html("<html><body>You can close this tab and return to Vox.</body></html>")
}

/// Run the full loopback PKCE flow against OpenRouter and return the
/// provisioned API key on success. Opens the system browser; blocks (async)
/// until the callback arrives or `CALLBACK_TIMEOUT` elapses.
pub async fn run_openrouter_flow() -> Result<String, OAuthError> {
    let PkcePair {
        verifier,
        challenge,
    } = pkce::generate();
    let state_value = pkce::generate_state();

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(OAuthError::Bind)?;
    let port = listener.local_addr().map_err(OAuthError::Bind)?.port();

    let callback_url = format!("http://127.0.0.1:{port}/callback");
    let auth_url = format!(
        "{OPENROUTER_AUTH_URL}?callback_url={}&code_challenge={}&code_challenge_method=S256&state={}",
        urlencoding_encode(&callback_url),
        challenge,
        state_value,
    );

    // Attempt to open the browser *before* spawning the loopback server (and
    // before the listener above is wrapped into one). If this fails, return
    // immediately with no server ever started — otherwise a failed
    // `open::that` would leave the just-spawned axum task listening forever
    // with no one left to receive the callback: the caller sees a
    // "browser failed to open" error whose fallback URL points at a socket
    // that can never complete the flow. The bound `listener` itself is
    // cheap (no task, no leaked resources) and is simply dropped on this
    // early return.
    open::that(&auth_url).map_err(|e| OAuthError::BrowserOpen {
        url: auth_url.clone(),
        source: e,
    })?;

    let (tx, rx) = oneshot::channel();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let callback_state = Arc::new(CallbackState {
        expected_state: state_value.clone(),
        tx: std::sync::Mutex::new(Some(tx)),
        shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
    });

    let app = Router::new()
        .route("/callback", get(callback_handler))
        .with_state(callback_state);

    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    let code = tokio::time::timeout(CALLBACK_TIMEOUT, rx)
        .await
        .map_err(|_| OAuthError::TimedOut(CALLBACK_TIMEOUT))?
        .map_err(|_| OAuthError::TokenExchange("callback channel closed unexpectedly".into()))??;

    // Wait for the server task's graceful shutdown to actually finish
    // (bounded — near-instant once shutdown_tx fired above) rather than
    // aborting it out from under an in-flight response.
    let _ = tokio::time::timeout(Duration::from_secs(5), server).await;

    exchange_code_at(OPENROUTER_TOKEN_EXCHANGE_URL, &code, &verifier).await
}

async fn exchange_code_at(url: &str, code: &str, verifier: &str) -> Result<String, OAuthError> {
    let client = vox_http_client::client_builder()
        .build()
        .map_err(|e| OAuthError::TokenExchange(e.to_string()))?;
    let resp = client
        .post(url)
        .json(&ExchangeRequest {
            code,
            code_verifier: verifier,
            code_challenge_method: "S256",
        })
        .send()
        .await
        .map_err(|e| OAuthError::TokenExchange(e.to_string()))?
        .error_for_status()
        .map_err(|e| OAuthError::TokenExchange(e.to_string()))?
        .json::<ExchangeResponse>()
        .await
        .map_err(|e| OAuthError::TokenExchange(e.to_string()))?;
    Ok(resp.key)
}

fn urlencoding_encode(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoding_escapes_colon_and_slash() {
        assert_eq!(
            urlencoding_encode("http://127.0.0.1:5555/callback"),
            "http%3A%2F%2F127.0.0.1%3A5555%2Fcallback"
        );
    }

    #[test]
    fn urlencoding_escapes_query_delimiters_the_hand_rolled_version_missed() {
        // The prior hand-rolled `s.replace(':', "%3A").replace('/', "%2F")`
        // only escaped 2 characters and would have let '&', '=', and spaces
        // pass through unescaped, corrupting a callback_url containing its
        // own query string. The real `urlencoding` crate handles these.
        assert_eq!(
            urlencoding_encode("http://127.0.0.1:5555/callback?a=1&b=2 c"),
            "http%3A%2F%2F127.0.0.1%3A5555%2Fcallback%3Fa%3D1%26b%3D2%20c"
        );
    }

    #[test]
    fn exchange_request_debug_redacts_code_verifier() {
        let req = ExchangeRequest {
            code: "auth-code-xyz",
            code_verifier: "super-secret-verifier-value",
            code_challenge_method: "S256",
        };
        let dbg = format!("{req:?}");
        assert!(
            !dbg.contains("super-secret-verifier-value"),
            "code_verifier must never appear in Debug: {dbg}"
        );
        assert!(
            !dbg.contains("auth-code-xyz"),
            "code (single-use, but still sensitive) must never appear in Debug: {dbg}"
        );
        assert!(dbg.contains("[redacted"), "should render redacted: {dbg}");
        assert!(
            dbg.contains("S256"),
            "non-secret code_challenge_method should stay visible: {dbg}"
        );
    }

    fn test_state(
        expected_state: &str,
        tx: oneshot::Sender<Result<String, OAuthError>>,
    ) -> Arc<CallbackState> {
        let (shutdown_tx, _shutdown_rx) = oneshot::channel();
        Arc::new(CallbackState {
            expected_state: expected_state.to_string(),
            tx: std::sync::Mutex::new(Some(tx)),
            shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
        })
    }

    #[tokio::test]
    async fn callback_handler_rejects_state_mismatch() {
        let (tx, rx) = oneshot::channel();
        let state = test_state("expected-123", tx);
        let query = Query(CallbackQuery {
            code: Some("some-code".to_string()),
            state: Some("wrong-state".to_string()),
        });
        let _ = callback_handler(State(state), query).await;
        let result = rx.await.expect("tx sent");
        assert!(matches!(result, Err(OAuthError::StateMismatch)));
    }

    #[tokio::test]
    async fn callback_handler_accepts_matching_state() {
        let (tx, rx) = oneshot::channel();
        let state = test_state("expected-123", tx);
        let query = Query(CallbackQuery {
            code: Some("real-code".to_string()),
            state: Some("expected-123".to_string()),
        });
        let _ = callback_handler(State(state), query).await;
        let result = rx.await.expect("tx sent");
        assert_eq!(result.unwrap(), "real-code");
    }

    #[tokio::test]
    async fn callback_handler_accepts_missing_state_param() {
        // Regression test for the state-leniency design decision: OpenRouter's
        // OAuth docs don't document echoing `state` back, so absence must not
        // be treated as a failure — only an explicit wrong value should be.
        let (tx, rx) = oneshot::channel();
        let state = test_state("expected-123", tx);
        let query = Query(CallbackQuery {
            code: Some("real-code".to_string()),
            state: None,
        });
        let _ = callback_handler(State(state), query).await;
        let result = rx.await.expect("tx sent");
        assert_eq!(result.unwrap(), "real-code");
    }

    #[tokio::test]
    async fn callback_handler_survives_poisoned_mutex() {
        // Regression test for the reachable `.expect("...poisoned")` fix:
        // poison `state.tx`'s mutex from another thread (panic while
        // holding the lock), then prove the handler still runs to
        // completion and delivers the result via the recovered guard,
        // instead of panicking itself.
        let (tx, rx) = oneshot::channel();
        let state = test_state("expected-123", tx);

        let poison_state = Arc::clone(&state);
        let poisoner = std::thread::spawn(move || {
            let _guard = poison_state.tx.lock().unwrap();
            panic!("intentionally poisoning the mutex for the regression test");
        });
        let _ = poisoner.join(); // join() returns Err on panic; that's expected.
        assert!(state.tx.is_poisoned());

        let query = Query(CallbackQuery {
            code: Some("real-code".to_string()),
            state: Some("expected-123".to_string()),
        });
        // Must not panic despite the poisoned mutex.
        let _ = callback_handler(State(state), query).await;
        let result = rx.await.expect("tx sent despite poisoned mutex");
        assert_eq!(result.unwrap(), "real-code");
    }

    #[tokio::test]
    async fn callback_handler_signals_shutdown_after_responding() {
        let (tx, rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let state = Arc::new(CallbackState {
            expected_state: "expected-123".to_string(),
            tx: std::sync::Mutex::new(Some(tx)),
            shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
        });
        let query = Query(CallbackQuery {
            code: Some("real-code".to_string()),
            state: Some("expected-123".to_string()),
        });
        let _ = callback_handler(State(state), query).await;
        let _ = rx.await.expect("tx sent");
        shutdown_rx
            .await
            .expect("shutdown signal sent after response was built");
    }

    #[tokio::test]
    async fn exchange_code_returns_key_on_success() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/v1/auth/keys"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"key": "sk-or-test-key"})),
            )
            .mount(&server)
            .await;

        let result = exchange_code_at(
            &format!("{}/api/v1/auth/keys", server.uri()),
            "test-code",
            "test-verifier",
        )
        .await;
        assert_eq!(result.unwrap(), "sk-or-test-key");
    }
}
