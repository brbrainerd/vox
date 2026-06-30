//! Auth middleware for the `vox share` reverse proxy.
//!
//! Three modes:
//! - `None` — no auth, public access
//! - `UrlToken(token)` — URL-embedded token (`?vox_share_token=<token>`)
//! - `Basic(user, pass)` — HTTP Basic auth
//!
//! The middleware is applied as an Axum `from_fn` layer on top of the proxy.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use rand::Rng;

/// Auth mode for a share session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMode {
    /// No authentication — public access.
    None,
    /// URL-embedded token. Public URL includes `?vox_share_token=<token>`.
    UrlToken(String),
    /// HTTP Basic authentication.
    Basic { user: String, pass: String },
}

impl AuthMode {
    /// Generate a random 16-hex-char URL token.
    pub fn random_token() -> Self {
        let bytes: [u8; 8] = rand::thread_rng().r#gen();
        Self::UrlToken(hex::encode(bytes))
    }

    /// Append auth to a base URL for user display.
    pub fn decorate_url(&self, base_url: &str) -> String {
        match self {
            Self::None | Self::Basic { .. } => base_url.to_string(),
            Self::UrlToken(token) => {
                if base_url.contains('?') {
                    format!("{}&vox_share_token={}", base_url, token)
                } else {
                    format!("{}?vox_share_token={}", base_url, token)
                }
            }
        }
    }
}

impl std::str::FromStr for AuthMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            s if s.starts_with("basic:") => {
                let rest = &s["basic:".len()..];
                if let Some((user, pass)) = rest.split_once(':') {
                    Ok(Self::Basic {
                        user: user.to_string(),
                        pass: pass.to_string(),
                    })
                } else {
                    Err(format!("basic auth format: `basic:user:pass`, got `{}`", s))
                }
            }
            _ => Err(format!(
                "unknown auth mode `{}`. Use: none, basic:user:pass",
                s
            )),
        }
    }
}

/// Axum middleware function that enforces the auth mode.
pub async fn auth_middleware(
    axum::extract::State(mode): axum::extract::State<AuthMode>,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    match &mode {
        AuthMode::None => Ok(next.run(req).await),
        AuthMode::UrlToken(expected) => {
            if token_present(req.headers(), req.uri().query(), expected) {
                Ok(next.run(req).await)
            } else {
                Ok(unauthorized_html(
                    "This vox share URL requires a valid token.",
                ))
            }
        }
        AuthMode::Basic { user, pass } => {
            if basic_auth_ok(req.headers(), user, pass) {
                Ok(next.run(req).await)
            } else {
                Ok(basic_auth_challenge())
            }
        }
    }
}

fn token_present(headers: &HeaderMap, query: Option<&str>, expected: &str) -> bool {
    // Check query param.
    if let Some(q) = query {
        for (k, v) in form_urlencoded::parse(q.as_bytes()) {
            if k == "vox_share_token" && v == expected {
                return true;
            }
        }
    }
    // Check cookie.
    if let Some(cookie_hdr) = headers.get(header::COOKIE) {
        if let Ok(cookie_str) = cookie_hdr.to_str() {
            for part in cookie_str.split(';') {
                let part = part.trim();
                if let Some(val) = part.strip_prefix("vox_share_token=") {
                    if val == expected {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn basic_auth_ok(headers: &HeaderMap, expected_user: &str, expected_pass: &str) -> bool {
    let Some(auth) = headers.get(header::AUTHORIZATION) else {
        return false;
    };
    let auth_str = match auth.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let Some(encoded) = auth_str.strip_prefix("Basic ") else {
        return false;
    };
    let decoded = match base64_decode(encoded) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let decoded_str = match std::str::from_utf8(&decoded) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let Some((u, p)) = decoded_str.split_once(':') else {
        return false;
    };
    u == expected_user && p == expected_pass
}

fn base64_decode(s: &str) -> Result<Vec<u8>, ()> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|_| ())
}

fn unauthorized_html(msg: &str) -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(format!(
            r#"<!DOCTYPE html><html><head><title>vox share — Access Denied</title></head>
<body><h1>Access Denied</h1><p>{}</p>
<p>Add <code>?vox_share_token=...</code> to the URL or use the full URL from <code>vox share</code>.</p>
</body></html>"#,
            msg
        )))
        .unwrap()
}

fn basic_auth_challenge() -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::WWW_AUTHENTICATE, r#"Basic realm="vox share""#)
        .body(Body::from("Authentication required"))
        .unwrap()
}
