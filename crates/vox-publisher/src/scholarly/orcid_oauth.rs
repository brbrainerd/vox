//! ORCID PKCE OAuth 2.0 state machine — pure types, no network I/O.
//!
//! Callers hold a `PkceVerifier`, build an `AuthorizationRequest`, redirect the user,
//! then exchange the code with a `TokenExchangeRequest`. The HTTP calls themselves
//! are the caller's responsibility (Phase 9 will wire them to reqwest).

use sha2::{Digest, Sha256};

/// Random high-entropy string generated before the OAuth redirect.
#[derive(Debug, Clone)]
pub struct PkceVerifier(pub String);

/// SHA-256 base64url-encoded challenge derived from the verifier.
#[derive(Debug, Clone)]
pub struct PkceChallenge(pub String);

impl PkceVerifier {
    /// Derive the PKCE challenge: SHA-256(verifier) → base64url without padding.
    pub fn to_challenge(&self) -> PkceChallenge {
        let hash = Sha256::digest(self.0.as_bytes());
        let encoded = base64url_no_pad(&hash);
        PkceChallenge(encoded)
    }
}

fn base64url_no_pad(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Parameters for building the ORCID authorization redirect URL.
#[derive(Debug, Clone)]
pub struct AuthorizationRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: String,
    pub code_challenge: PkceChallenge,
}

impl AuthorizationRequest {
    /// Build the full authorization URL for the user redirect.
    pub fn authorization_url(&self, authorize_endpoint: &str) -> String {
        format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}\
             &code_challenge={}&code_challenge_method=S256",
            authorize_endpoint,
            percent_encode(&self.client_id),
            percent_encode(&self.redirect_uri),
            percent_encode(&self.scope),
            percent_encode(&self.state),
            percent_encode(&self.code_challenge.0),
        )
    }
}

/// Parameters for the token exchange POST after the user authorizes.
#[derive(Debug, Clone)]
pub struct TokenExchangeRequest {
    pub client_id: String,
    pub client_secret: String,
    pub code: String,
    pub redirect_uri: String,
    pub code_verifier: PkceVerifier,
}

impl TokenExchangeRequest {
    /// Encode as `application/x-www-form-urlencoded` body.
    pub fn to_form_body(&self) -> String {
        format!(
            "grant_type=authorization_code&client_id={}&client_secret={}\
             &code={}&redirect_uri={}&code_verifier={}",
            percent_encode(&self.client_id),
            percent_encode(&self.client_secret),
            percent_encode(&self.code),
            percent_encode(&self.redirect_uri),
            percent_encode(&self.code_verifier.0),
        )
    }
}

/// Decoded ORCID access token response.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OrcidToken {
    pub access_token: String,
    pub token_type: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
    pub orcid: Option<String>,
    pub name: Option<String>,
}

fn percent_encode(s: &str) -> String {
    // RFC 3986 unreserved set (A-Za-z0-9-_.~), uppercase %XX for the rest —
    // exactly what `urlencoding::encode` produces. (Not `form_urlencoded`,
    // which would encode space as `+`.)
    urlencoding::encode(s).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_base64url_sha256_of_verifier() {
        let verifier = PkceVerifier("abc123".to_string());
        let challenge = verifier.to_challenge();
        assert!(!challenge.0.contains('+'));
        assert!(!challenge.0.contains('/'));
        assert!(!challenge.0.contains('='));
        assert!(!challenge.0.is_empty());
    }

    #[test]
    fn authorization_url_contains_client_id_and_challenge() {
        let req = AuthorizationRequest {
            client_id: "APP-TEST123".to_string(),
            redirect_uri: "https://localhost/callback".to_string(),
            scope: "openid /authenticate".to_string(),
            state: "state-xyz".to_string(),
            code_challenge: PkceChallenge("challenge-abc".to_string()),
        };
        let url = req.authorization_url("https://orcid.org/oauth/authorize");
        assert!(url.contains("APP-TEST123"));
        assert!(url.contains("challenge-abc"));
        assert!(url.contains("S256"));
    }

    #[test]
    fn token_exchange_body_contains_verifier() {
        let req = TokenExchangeRequest {
            client_id: "APP-TEST123".to_string(),
            client_secret: "secret".to_string(),
            code: "auth-code".to_string(),
            redirect_uri: "https://localhost/callback".to_string(),
            code_verifier: PkceVerifier("verifier-xyz".to_string()),
        };
        let body = req.to_form_body();
        assert!(body.contains("verifier-xyz"));
        assert!(body.contains("auth-code"));
        assert!(body.contains("authorization_code"));
    }
}
