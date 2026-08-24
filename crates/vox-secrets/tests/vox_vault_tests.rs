//! Vault integration tests; only meaningful when the vault backend is compiled in.
#![cfg(feature = "secrets-vox-vault")]

use secrecy::ExposeSecret;
use vox_secrets::backend::SecretBackend;
use vox_secrets::backend::vox_vault::VoxCloudBackend;
use vox_secrets::spec::{SecretId, SecretSpec};

#[test]
fn test_vox_vault_encryption_decryption_cycle() {
    // If the keyring cannot be acquired in pure headless CI, VoxCloudBackend::new() returns an error.
    // So we handle the Result gracefully to ensure this test passes locally when keyring is available.
    let backend = match VoxCloudBackend::new() {
        Ok(b) => b,
        Err(_) => {
            println!("Skipping VoxCloudBackend test because keyring or db is not available");
            return;
        }
    };

    let spec = SecretSpec {
        id: SecretId::CustomOpenaiApiKey,
        canonical_env: "FAKE_TARGET_TEST",
        aliases: &[],
        deprecated_aliases: &[],
        backend_key: None,
        auth_registry: None,
        policy: vox_secrets::policy::SecretPolicy::required_fail(),
        remediation: "",
        scope_description: "",
    };

    let plaintext = "vox_vault_test_secret_12345";

    // Test write
    backend
        .write_secret("FAKE_TARGET_TEST", plaintext)
        .expect("failed to write secret to vault");

    // Test read
    let resolved = backend
        .resolve(SecretId::CustomOpenaiApiKey, spec, None, "test")
        .expect("failed to resolve secret from vault")
        .expect("secret not found after write");

    assert_eq!(resolved.expose_secret(), plaintext);
}

#[test]
fn test_vox_vault_rewrap_and_backup_corruption_detection() {
    let backend = match VoxCloudBackend::new() {
        Ok(b) => b,
        Err(_) => {
            println!("Skipping VoxCloudBackend test because keyring or db is not available");
            return;
        }
    };
    backend
        .write_secret("FAKE_TARGET_REWRAP", "rewrap_plaintext")
        .expect("seed secret");
    let rewrapped = backend
        .rewrap_secret("FAKE_TARGET_REWRAP", "kek-rotated", 2)
        .expect("rewrap call");
    assert!(rewrapped, "rewrap should mutate existing row");

    let backup = backend
        .export_account_backup(
            &std::env::var("VOX_ACCOUNT_ID").unwrap_or_else(|_| "default-account".to_string()),
        )
        .expect("export backup");
    assert!(!backup.is_empty(), "backup should include seeded row");
    let mut corrupted = backup.clone();
    corrupted[0].ciphertext[0] ^= 0x01;
    let err = backend
        .import_account_backup(&corrupted, true)
        .expect_err("corrupted backup must fail integrity check");
    assert!(
        err.to_string().contains("checksum mismatch"),
        "error must mention checksum mismatch"
    );
}

#[test]
fn test_rewrap_rotation_across_secret_material_kinds() {
    let backend = match VoxCloudBackend::new() {
        Ok(b) => b,
        Err(_) => {
            println!("Skipping VoxCloudBackend test because keyring or db is not available");
            return;
        }
    };
    let cases = [
        (
            SecretId::CustomOpenaiApiKey,
            "ROTATE_API_KEY_KIND",
            "kind-api-key-value",
        ),
        (
            SecretId::VoxOpenReviewAccessToken,
            "ROTATE_BEARER_TOKEN_KIND",
            "kind-bearer-token-value",
        ),
        (
            SecretId::VoxOpenReviewPassword,
            "ROTATE_PASSWORD_KIND",
            "kind-password-value",
        ),
    ];
    for (id, key, value) in cases {
        backend.write_secret(key, value).expect("seed secret");
        let rotated = backend
            .rewrap_secret(key, "kek-rotation-suite", 3)
            .expect("rewrap");
        assert!(rotated);
        let spec = SecretSpec {
            id,
            canonical_env: key,
            aliases: &[],
            deprecated_aliases: &[],
            backend_key: None,
            auth_registry: None,
            policy: vox_secrets::policy::SecretPolicy::required_fail(),
            remediation: "",
            scope_description: "",
        };
        let resolved = backend
            .resolve(id, spec, None, "test")
            .expect("resolve after rewrap")
            .expect("secret exists");
        assert_eq!(resolved.expose_secret(), value);
    }
}
