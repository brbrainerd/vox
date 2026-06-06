//! P1 Task 2 — store ops for `user_identities` and `scientia_nanopubs`.
//!
//! Verifies the per-user nanopub identity binding (design §4.1) round-trips:
//! upsert + read-back of a `user_identities` row, then insert + fetch-by-trusty-uri
//! of a `scientia_nanopubs` row.

use vox_db::store::{NanopubRow, UserIdentityRow};
use vox_db::{DbConfig, VoxDb};

#[tokio::test]
async fn user_identity_upsert_round_trip() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let row = UserIdentityRow {
        user_id: "local-user".into(),
        orcid_id: Some("https://orcid.org/0000-0002-1825-0097".into()),
        nanopub_pubkey_b64: Some("MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA_fake_b64".into()),
        nanopub_key_ref: "VOX_USER_RSA_NANOPUB_PRIVATE_KEY_B64".into(),
        created_at_ms: 1_747_000_000_000,
        updated_at_ms: 1_747_000_000_000,
    };
    db.upsert_user_identity(&row).await.expect("upsert");

    let got = db
        .get_user_identity("local-user")
        .await
        .expect("get")
        .expect("row present");
    assert_eq!(got, row);
}

#[tokio::test]
async fn user_identity_upsert_overwrites_existing() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let mut row = UserIdentityRow {
        user_id: "local-user".into(),
        orcid_id: None,
        nanopub_pubkey_b64: None,
        nanopub_key_ref: "VOX_USER_RSA_NANOPUB_PRIVATE_KEY_B64".into(),
        created_at_ms: 100,
        updated_at_ms: 100,
    };
    db.upsert_user_identity(&row).await.expect("insert");

    row.orcid_id = Some("https://orcid.org/0000-0002-1825-0097".into());
    row.updated_at_ms = 200;
    db.upsert_user_identity(&row).await.expect("update");

    let got = db
        .get_user_identity("local-user")
        .await
        .expect("get")
        .expect("present");
    assert_eq!(
        got.orcid_id.as_deref(),
        Some("https://orcid.org/0000-0002-1825-0097")
    );
    assert_eq!(got.updated_at_ms, 200);
}

#[tokio::test]
async fn get_user_identity_returns_none_when_absent() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let got = db.get_user_identity("nobody").await.expect("get");
    assert!(got.is_none());
}

#[tokio::test]
async fn nanopub_insert_then_fetch_by_trusty_uri() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let row = NanopubRow {
        trusty_uri: "https://w3id.org/np/RAxampleTrustyURIhash".into(),
        claim_id: 42,
        publication_id: Some("pub-001".into()),
        user_id: "local-user".into(),
        orcid_id: Some("https://orcid.org/0000-0002-1825-0097".into()),
        trig: "@prefix this: <https://w3id.org/np/RAx> .".into(),
        validated_offline: false,
        published_state: "local".into(),
        created_at_ms: 1_747_000_000_000,
    };
    db.insert_scientia_nanopub(&row).await.expect("insert");

    let got = db
        .get_nanopub_by_trusty_uri("https://w3id.org/np/RAxampleTrustyURIhash")
        .await
        .expect("get")
        .expect("row present");
    assert_eq!(got, row);
}

#[tokio::test]
async fn get_nanopub_returns_none_when_absent() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let got = db
        .get_nanopub_by_trusty_uri("https://w3id.org/np/missing")
        .await
        .expect("get");
    assert!(got.is_none());
}
