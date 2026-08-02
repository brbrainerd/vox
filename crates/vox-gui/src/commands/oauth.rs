//! Tauri commands for in-app OAuth key provisioning (free-tier onboarding).

use serde::Serialize;
use tauri::{command, AppHandle, Manager};
use vox_oauth_pkce::openrouter::OAuthError;
use vox_secrets::SecretError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthLoginResultDto {
    pub success: bool,
    pub error: Option<String>,
    /// Set only when the browser failed to open automatically — lets the
    /// caller show a copyable/clickable fallback link instead of dead-ending.
    pub fallback_url: Option<String>,
}

/// Map the OAuth flow's own error into the DTO's error/fallback_url pair.
/// Pure, no I/O — independently testable without a live browser.
fn map_flow_error(e: &OAuthError) -> (String, Option<String>) {
    match e {
        OAuthError::BrowserOpen { url, .. } => (e.to_string(), Some(url.clone())),
        _ => (e.to_string(), None),
    }
}

/// Map `set_registry_token`'s real return type (`Result<PathBuf, SecretError>`
/// — not `Result<(), _>`) into the DTO. Pure, no I/O beyond what the caller
/// already did — independently testable with a pre-computed `Result`.
fn map_store_result(result: Result<std::path::PathBuf, SecretError>) -> OAuthLoginResultDto {
    match result {
        Ok(_path) => OAuthLoginResultDto {
            success: true,
            error: None,
            fallback_url: None,
        },
        Err(e) => OAuthLoginResultDto {
            success: false,
            error: Some(format!("failed to store key: {e}")),
            fallback_url: None,
        },
    }
}

/// Run the OpenRouter loopback OAuth flow and persist the resulting key via
/// the same storage path `set_secret` already uses for OPENROUTER_API_KEY
/// (`vox_secrets::set_registry_token("openrouter", ...)`), so the GUI's
/// `list_secret_status`/`vox doctor` see it identically to a manually-entered key.
#[command]
pub async fn oauth_login_openrouter(app: AppHandle) -> OAuthLoginResultDto {
    match vox_oauth_pkce::openrouter::run_openrouter_flow().await {
        Ok(key) => {
            let result = map_store_result(vox_secrets::set_registry_token("openrouter", &key, None));
            if result.success
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = window.set_focus();
            }
            result
        }
        Err(e) => {
            let (error, fallback_url) = map_flow_error(&e);
            OAuthLoginResultDto {
                success: false,
                error: Some(error),
                fallback_url,
            }
        }
    }
}

/// OpenRouter's documented "get current API key" endpoint — a cheap,
/// read-only, key-scoped GET that requires auth (proves the key works)
/// without consuming a completion/costing money. Confirmed against
/// OpenRouter's live docs (https://openrouter.ai/docs/api-reference/limits):
/// `GET {base}/api/v1/key` with `Authorization: Bearer <key>`, 200 on a
/// valid key, 401 on missing/invalid/disabled/expired credentials.
const OPENROUTER_API_BASE_URL: &str = "https://openrouter.ai";

/// Hit the key-info endpoint at `base_url` and report whether `key` is
/// live. Fails closed: any transport/build error (DNS failure, connection
/// refused, TLS error, etc.) or a non-2xx response (401 for a bad key)
/// yields `false` rather than propagating an error — callers only need a
/// yes/no "is this key actually usable" answer. `base_url` is swappable so
/// tests can point this at a `wiremock::MockServer` instead of the real
/// OpenRouter host.
async fn verify_key_at(base_url: &str, key: &str) -> bool {
    let Ok(client) = vox_http_client::client_builder().build() else {
        return false;
    };
    let url = format!("{base_url}/api/v1/key");
    match client.get(url).bearer_auth(key).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Real post-OAuth connectivity check: resolve the stored OpenRouter key
/// (via the same `vox_secrets` resolution path `list_secret_status`/`vox
/// doctor` use) and confirm it's actually accepted by OpenRouter, not just
/// "a string got stored". No key present, an empty key, and any network
/// error are all treated identically as `false` (fail closed) — there is
/// nothing to verify successfully in any of those cases.
#[command]
pub async fn verify_openrouter_key() -> bool {
    let resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey);
    match resolved.expose() {
        Some(key) if !key.is_empty() => verify_key_at(OPENROUTER_API_BASE_URL, key).await,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_dto_serializes_camel_case() {
        let dto = OAuthLoginResultDto {
            success: false,
            error: Some("timed out".to_string()),
            fallback_url: None,
        };
        let json = serde_json::to_string(&dto).expect("serializes");
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"timed out\""));
    }

    #[test]
    fn map_store_result_ok_path_is_success() {
        // The real bug this test guards against: an earlier draft matched
        // `Ok(())` against `set_registry_token`'s actual `Result<PathBuf,_>`
        // return type, which does not compile. This asserts the PathBuf
        // case is handled, not discarded/mismatched.
        let dto = map_store_result(Ok(std::path::PathBuf::from("/fake/path")));
        assert!(dto.success);
        assert!(dto.error.is_none());
    }

    #[test]
    fn map_store_result_err_is_failure_with_message() {
        let dto = map_store_result(Err(SecretError::Io("disk full".to_string())));
        assert!(!dto.success);
        assert!(dto.error.unwrap().contains("disk full"));
    }

    #[test]
    fn map_flow_error_browser_open_carries_fallback_url() {
        let err = OAuthError::BrowserOpen {
            url: "https://openrouter.ai/auth?callback_url=...".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "no browser"),
        };
        let (_, fallback_url) = map_flow_error(&err);
        assert_eq!(fallback_url.as_deref(), Some("https://openrouter.ai/auth?callback_url=..."));
    }

    #[test]
    fn map_flow_error_timeout_has_no_fallback_url() {
        let err = OAuthError::TimedOut(std::time::Duration::from_secs(120));
        let (_, fallback_url) = map_flow_error(&err);
        assert!(fallback_url.is_none());
    }
}

#[cfg(test)]
mod verify_tests {
    use super::*;

    #[tokio::test]
    async fn verify_returns_true_on_200() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v1/key"))
            .and(wiremock::matchers::header("Authorization", "Bearer fake-key"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "label": "test",
                        "limit": null,
                        "limit_remaining": null,
                        "usage": 0,
                        "is_free_tier": true
                    }
                })),
            )
            .mount(&server)
            .await;

        assert!(verify_key_at(&server.uri(), "fake-key").await);
    }

    #[tokio::test]
    async fn verify_returns_false_on_401() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v1/key"))
            .respond_with(wiremock::ResponseTemplate::new(401))
            .mount(&server)
            .await;

        assert!(!verify_key_at(&server.uri(), "revoked-or-bad-key").await);
    }

    #[tokio::test]
    async fn verify_returns_false_on_network_error() {
        // Nothing listening on this port — fails closed rather than panicking
        // or propagating an error out of a bool-returning fn.
        assert!(!verify_key_at("http://127.0.0.1:1", "any-key").await);
    }
}
