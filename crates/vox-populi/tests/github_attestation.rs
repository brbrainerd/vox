//! GitHub attestation manifest + device-flow + revocation tests (P5-T2).

use vox_crypto::{generate_signing_keypair, verifying_key_to_bytes};
use vox_populi::pairing::github_attestation::{AttestationManifest, ManifestVerifyError};

// ── P5-T2a: manifest sign/verify ─────────────────────────────────────────────

#[test]
fn manifest_round_trip_verifies() {
    let (sk, vk) = generate_signing_keypair();
    let manifest = AttestationManifest::new_signed(
        &hex::encode(verifying_key_to_bytes(&vk)),
        "12345",
        "alice",
        1_900_000_000_000,
        &sk,
        &vk,
    );
    let unverified = serde_json::to_string(&manifest).unwrap();
    let parsed: AttestationManifest = serde_json::from_str(&unverified).unwrap();
    assert!(parsed.verify().is_ok());
}

#[test]
fn manifest_with_swapped_pubkey_is_rejected() {
    let (sk, vk) = generate_signing_keypair();
    let (_, vk_other) = generate_signing_keypair();
    let mut manifest = AttestationManifest::new_signed(
        &hex::encode(verifying_key_to_bytes(&vk)),
        "12345",
        "alice",
        1_900_000_000_000,
        &sk,
        &vk,
    );
    manifest.node_pubkey_hex = hex::encode(verifying_key_to_bytes(&vk_other));
    assert!(matches!(
        manifest.verify().unwrap_err(),
        ManifestVerifyError::SignatureMismatch
    ));
}

#[test]
fn manifest_expired_is_rejected() {
    let (sk, vk) = generate_signing_keypair();
    let manifest = AttestationManifest::new_signed(
        &hex::encode(verifying_key_to_bytes(&vk)),
        "12345",
        "alice",
        1, // 1ms after epoch — expired
        &sk,
        &vk,
    );
    assert!(matches!(
        manifest.verify().unwrap_err(),
        ManifestVerifyError::Expired { .. }
    ));
}

// ── P5-T2b: device-flow round-trip ───────────────────────────────────────────

#[tokio::test]
async fn device_flow_round_trip_with_mock() {
    use vox_populi::pairing::device_flow::{DeviceFlow, DeviceFlowConfig};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login/device/code"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("content-type", "application/json")
                .set_body_string(
                    r#"{"device_code":"DC","user_code":"UC","verification_uri":"https://x","expires_in":900,"interval":1}"#,
                ),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"access_token":"AT","token_type":"bearer","scope":"gist"}"#),
        )
        .mount(&server)
        .await;

    let cfg = DeviceFlowConfig {
        client_id: "test-client".into(),
        github_login_base: server.uri(),
        github_api_base: "https://api.github.com".into(),
        scope: "gist".into(),
        poll_interval_ms: 10,
    };
    let flow = DeviceFlow::new(cfg);
    let init = flow.start().await.expect("start");
    assert_eq!(init.user_code, "UC");
    let token = flow.poll_until_token(&init).await.expect("token");
    assert_eq!(token, "AT");
}

// ── P5-T2c: fetch + verify + revocation ──────────────────────────────────────

#[tokio::test]
async fn counterparty_fetches_and_verifies_manifest() {
    use vox_populi::pairing::github_attestation::fetch_and_verify;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let (sk, vk) = generate_signing_keypair();
    let pubkey_hex = hex::encode(verifying_key_to_bytes(&vk));
    let manifest =
        AttestationManifest::new_signed(&pubkey_hex, "12345", "alice", 1_900_000_000_000, &sk, &vk);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/raw/manifest.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("content-type", "application/json")
                .set_body_string(serde_json::to_string(&manifest).unwrap()),
        )
        .mount(&server)
        .await;
    let url = format!("{}/raw/manifest.json", server.uri());
    let admitted = fetch_and_verify(&url).await.expect("admit");
    assert_eq!(admitted.github_login, "alice");
}

#[test]
fn revoked_manifest_is_tombstoned_within_60_seconds() {
    use vox_populi::pairing::revocation::RevocationGossip;

    let mut rg = RevocationGossip::new(vox_config::timeouts::D_60S);
    rg.tombstone("nodeA-pubkey-hex".into());
    assert!(rg.is_revoked("nodeA-pubkey-hex"));
    assert!(!rg.is_revoked("nodeB-pubkey-hex"));
}
