//! Webhook signature generation and verification using HMAC-SHA3-256.

use data_encoding::HEXLOWER;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest as _, Sha256};
use sha3::Sha3_256;

use super::WebhookError;

/// A webhook signature — an HMAC-SHA3-256 or HMAC-SHA256 hex digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookSignature(pub String);

impl std::fmt::Display for WebhookSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Sign a payload with a secret key using HMAC-SHA3-256.
pub fn sign_payload(secret: &str, payload: &[u8]) -> WebhookSignature {
    let key = secret.as_bytes();
    let block_size = 136usize; // SHA3-256 block = 1088 bits = 136 bytes

    let mut padded_key = [0u8; 136];
    let key_to_use = if key.len() > block_size {
        let mut h = Sha3_256::new();
        h.update(key);
        let hashed = h.finalize();
        padded_key[..32].copy_from_slice(&hashed);
        &padded_key[..block_size]
    } else {
        padded_key[..key.len()].copy_from_slice(key);
        &padded_key[..block_size]
    };

    let mut ipad_key = [0u8; 136];
    let mut opad_key = [0u8; 136];
    for i in 0..block_size {
        ipad_key[i] = key_to_use[i] ^ 0x36;
        opad_key[i] = key_to_use[i] ^ 0x5c;
    }

    let mut inner = Sha3_256::new();
    inner.update(ipad_key);
    inner.update(payload);
    let inner_hash = inner.finalize();

    let mut outer = Sha3_256::new();
    outer.update(opad_key);
    outer.update(inner_hash);
    let result = outer.finalize();

    WebhookSignature(format!("sha3={}", HEXLOWER.encode(&result)))
}

/// Sign a payload with a secret key using standard HMAC-SHA256 (for Slack/GitHub).
pub fn sign_hmac_sha256(secret: &str, payload: &[u8]) -> WebhookSignature {
    let key = secret.as_bytes();
    let block_size = 64usize; // SHA-256 block = 512 bits = 64 bytes

    let mut padded_key = [0u8; 64];
    let key_to_use = if key.len() > block_size {
        let mut h = Sha256::new();
        h.update(key);
        let hashed = h.finalize();
        padded_key[..32].copy_from_slice(&hashed);
        &padded_key[..block_size]
    } else {
        padded_key[..key.len()].copy_from_slice(key);
        &padded_key[..block_size]
    };

    let mut ipad_key = [0u8; 64];
    let mut opad_key = [0u8; 64];
    for i in 0..block_size {
        ipad_key[i] = key_to_use[i] ^ 0x36;
        opad_key[i] = key_to_use[i] ^ 0x5c;
    }

    let mut inner = Sha256::new();
    inner.update(ipad_key);
    inner.update(payload);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad_key);
    outer.update(inner_hash);
    let result = outer.finalize();

    WebhookSignature(HEXLOWER.encode(&result))
}

/// Maximum allowed age (seconds) for a webhook timestamp before it is rejected
/// as a potential replay. Override at runtime via the
/// `VOX_WEBHOOK_REPLAY_WINDOW_SECS` env var (parsed as `u64`); defaults to
/// 300 (5 minutes), matching Slack's published guidance.
pub const DEFAULT_REPLAY_WINDOW_SECS: u64 = 300;

fn replay_window_secs() -> u64 {
    std::env::var("VOX_WEBHOOK_REPLAY_WINDOW_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_REPLAY_WINDOW_SECS)
}

/// Validate that `timestamp` is present, non-empty, parses as a Unix-epoch
/// integer, and is within the configured replay window relative to `now_unix`.
/// Returns the trimmed timestamp string on success.
fn require_fresh_timestamp(
    timestamp: &Option<String>,
    now_unix: u64,
) -> Result<&str, WebhookError> {
    let ts = timestamp
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(WebhookError::MissingTimestamp)?;
    let parsed = ts
        .parse::<u64>()
        .map_err(|_| WebhookError::TimestampOutOfWindow(ts.to_string()))?;
    let window = replay_window_secs();
    let abs_skew = parsed.abs_diff(now_unix);
    if abs_skew > window {
        return Err(WebhookError::TimestampOutOfWindow(ts.to_string()));
    }
    Ok(ts)
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Verify a payload against a generic or source-specific signature scheme.
///
/// Sources that bind a timestamp into the signed message (`discord`, `slack`)
/// also enforce a replay window via the private `require_fresh_timestamp`; an empty,
/// non-numeric, or stale timestamp is now a hard rejection rather than being
/// silently coerced to the empty string. This closes a replay/signature-bypass
/// hole where attackers could resubmit captured webhooks indefinitely.
pub fn verify_payload(
    secret: &str,
    payload: &[u8],
    signature: &str,
    timestamp: &Option<String>,
    source: &str,
) -> Result<(), WebhookError> {
    match source {
        "discord" => {
            // Discord signature is a hex-encoded Ed25519 signature
            // payload for Ed25519 verification is (timestamp + body).
            let ts = require_fresh_timestamp(timestamp, now_unix())?;
            let mut message = ts.as_bytes().to_vec();
            message.extend_from_slice(payload);

            let public_key_bytes = HEXLOWER
                .decode(secret.as_bytes())
                .map_err(|_| WebhookError::InvalidSignature)?;
            let mut pk_arr = [0u8; 32];
            if public_key_bytes.len() != 32 {
                return Err(WebhookError::InvalidSignature);
            }
            pk_arr.copy_from_slice(&public_key_bytes);

            let sig_bytes = HEXLOWER
                .decode(signature.as_bytes())
                .map_err(|_| WebhookError::InvalidSignature)?;
            let mut sig_arr = [0u8; 64];
            if sig_bytes.len() != 64 {
                return Err(WebhookError::InvalidSignature);
            }
            sig_arr.copy_from_slice(&sig_bytes);

            let public_key =
                VerifyingKey::from_bytes(&pk_arr).map_err(|_| WebhookError::InvalidSignature)?;
            let ed_sig = Signature::from_bytes(&sig_arr);

            public_key
                .verify(&message, &ed_sig)
                .map_err(|_| WebhookError::InvalidSignature)?;
            Ok(())
        }
        "slack" => {
            // Slack HMACS (v0:timestamp:payload)
            let ts = require_fresh_timestamp(timestamp, now_unix())?;
            let ts_prefix = format!("v0:{ts}:");
            let mut message = ts_prefix.as_bytes().to_vec();
            message.extend_from_slice(payload);

            let expected = sign_hmac_sha256(secret, &message);
            let expected_full = format!("v0={}", expected.0);

            if constant_time_eq(expected_full.as_bytes(), signature.as_bytes()) {
                Ok(())
            } else {
                Err(WebhookError::InvalidSignature)
            }
        }
        "github" => {
            // GitHub HMAC-SHA256 (sha256=...)
            let expected = sign_hmac_sha256(secret, payload);
            let provided = signature.trim_start_matches("sha256=");
            if constant_time_eq(expected.0.as_bytes(), provided.as_bytes()) {
                Ok(())
            } else {
                Err(WebhookError::InvalidSignature)
            }
        }
        _ => {
            // Default generic fallback (SHA3-256)
            let expected = sign_payload(secret, payload);
            if constant_time_eq(expected.0.as_bytes(), signature.as_bytes()) {
                Ok(())
            } else {
                Err(WebhookError::InvalidSignature)
            }
        }
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify() {
        let signing_key = "my-webhook-secret";
        let payload = b"hello world";
        let sig = sign_payload(signing_key, payload);
        assert!(verify_payload(signing_key, payload, &sig.to_string(), &None, "custom").is_ok());
    }

    #[test]
    fn wrong_secret_fails_verification() {
        let sig = sign_payload("correct-secret", b"data");
        let result = verify_payload("wrong-secret", b"data", &sig.to_string(), &None, "custom");
        assert!(result.is_err());
    }

    #[test]
    fn tampered_payload_fails_verification() {
        let sig = sign_payload("secret", b"original");
        let result = verify_payload("secret", b"tampered", &sig.to_string(), &None, "custom");
        assert!(result.is_err());
    }

    #[test]
    fn signature_is_deterministic() {
        let sig1 = sign_payload("s", b"p");
        let sig2 = sign_payload("s", b"p");
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn slack_verify_rejects_missing_timestamp() {
        // Even a "valid" HMAC must not pass when the timestamp header is
        // absent — without it there is no replay defense at all.
        let payload = b"hello";
        let secret = "s";
        let result = verify_payload(secret, payload, "v0=anything", &None, "slack");
        assert!(matches!(result, Err(WebhookError::MissingTimestamp)));
    }

    #[test]
    fn slack_verify_rejects_empty_timestamp() {
        let result = verify_payload("s", b"hello", "v0=anything", &Some(String::new()), "slack");
        assert!(matches!(result, Err(WebhookError::MissingTimestamp)));
    }

    #[test]
    fn slack_verify_rejects_non_numeric_timestamp() {
        let result = verify_payload(
            "s",
            b"hello",
            "v0=anything",
            &Some("not-a-number".to_string()),
            "slack",
        );
        assert!(matches!(result, Err(WebhookError::TimestampOutOfWindow(_))));
    }

    #[test]
    fn slack_verify_rejects_stale_timestamp() {
        // 1970-01-01 — well outside any sane replay window.
        let result = verify_payload(
            "s",
            b"hello",
            "v0=anything",
            &Some("0".to_string()),
            "slack",
        );
        assert!(matches!(result, Err(WebhookError::TimestampOutOfWindow(_))));
    }

    #[test]
    fn discord_verify_rejects_missing_timestamp() {
        let result = verify_payload("aa", b"hello", "bb", &None, "discord");
        assert!(matches!(result, Err(WebhookError::MissingTimestamp)));
    }

    #[test]
    fn require_fresh_timestamp_accepts_within_window() {
        let now: u64 = 1_700_000_000;
        let ts = (now - 60).to_string();
        let arg = Some(ts.clone());
        let res = require_fresh_timestamp(&arg, now).unwrap();
        assert_eq!(res, ts);
    }

    #[test]
    fn require_fresh_timestamp_rejects_future_skew_outside_window() {
        let now: u64 = 1_700_000_000;
        let ts = (now + DEFAULT_REPLAY_WINDOW_SECS + 1).to_string();
        let err = require_fresh_timestamp(&Some(ts), now).unwrap_err();
        assert!(matches!(err, WebhookError::TimestampOutOfWindow(_)));
    }
}
