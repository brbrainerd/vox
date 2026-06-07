use crate::NodeIdentity;
use anyhow::Result;
use argon2::Argon2;
use rand::RngCore;
use std::fs;
use std::path::PathBuf;
use vox_crypto::{SymKey, decrypt_with_nonce, encrypt_with_nonce};

pub fn identity_key_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".vox");
    path.push("identity.key.enc");
    path
}

fn derive_key(password: &str, salt: &[u8]) -> Result<SymKey> {
    let mut key = [0u8; 32];
    let argon2 = Argon2::default();
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow::anyhow!("KDF failed: {}", e))?;
    Ok(SymKey(key))
}

pub fn save_identity(identity: &NodeIdentity, password: &str) -> Result<()> {
    save_identity_at(identity, password, &identity_key_path())
}

/// Encrypt and persist `identity` to an explicit `path`. Path-parameterized so
/// callers (and tests) can target a location other than `~/.vox/identity.key.enc`.
pub fn save_identity_at(
    identity: &NodeIdentity,
    password: &str,
    path: &std::path::Path,
) -> Result<()> {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);

    let sym_key = derive_key(password, &salt)?;

    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);

    let raw_signing_key = identity.signing_key().inner.to_bytes();
    let ciphertext =
        encrypt_with_nonce(&sym_key, &nonce, &raw_signing_key).map_err(|e| anyhow::anyhow!(e))?;

    let mut payload = Vec::new();
    payload.extend_from_slice(&salt);
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&ciphertext);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Explicitly setting permissions to 600 would be done here for Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true).mode(0o600);
        let mut file = options.open(&path)?;
        use std::io::Write;
        file.write_all(&payload)?;
    }
    #[cfg(not(unix))]
    {
        fs::write(&path, payload)?;
    }

    Ok(())
}

pub fn load_identity(password: &str) -> Result<NodeIdentity> {
    load_identity_at(password, &identity_key_path())
}

/// Decrypt and load an identity from an explicit `path`.
pub fn load_identity_at(password: &str, path: &std::path::Path) -> Result<NodeIdentity> {
    if !path.exists() {
        return Err(anyhow::anyhow!("Identity file not found at {:?}", path));
    }

    let payload = fs::read(&path)?;
    if payload.len() < 16 + 12 + 32 {
        return Err(anyhow::anyhow!("Identity file corrupted or too short"));
    }

    let salt = &payload[0..16];
    let nonce = &payload[16..28];
    let ciphertext = &payload[28..];

    let sym_key = derive_key(password, salt)?;

    let raw_signing_key = decrypt_with_nonce(&sym_key, nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

    if raw_signing_key.len() != 32 {
        return Err(anyhow::anyhow!(
            "Invalid signing key length after decryption"
        ));
    }

    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&raw_signing_key);

    let signing_key = vox_crypto::signing_key_from_bytes(&bytes);
    let verifying_key = vox_crypto::to_verifying_key(&signing_key);

    Ok(NodeIdentity::from_keys(signing_key, verifying_key))
}

/// Whether a local node identity exists on disk.
pub fn identity_exists() -> bool {
    identity_key_path().exists()
}

/// Rotate the local node identity: verify `password` unlocks the existing file
/// (when present), generate a fresh Ed25519 keypair, and re-encrypt it under the
/// SAME password at the canonical path. Returns the new identity.
///
/// Rotation requires the master password by design — the signing key is only
/// ever stored Argon2-encrypted, so there is no passwordless rotation path.
pub fn rotate_identity(password: &str) -> Result<NodeIdentity> {
    rotate_identity_at(password, &identity_key_path())
}

/// Path-parameterized rotation (see [`rotate_identity`]). Hermetic for tests.
pub fn rotate_identity_at(password: &str, path: &std::path::Path) -> Result<NodeIdentity> {
    if password.is_empty() {
        return Err(anyhow::anyhow!("Master password cannot be empty"));
    }
    // If an identity already exists, the supplied password MUST unlock it before
    // we overwrite — this prevents an attacker with file write access (but no
    // password) from silently replacing the node's identity.
    if path.exists() {
        load_identity_at(password, path)
            .map_err(|_| anyhow::anyhow!("Incorrect master password — rotation refused"))?;
    }
    let fresh = NodeIdentity::generate();
    save_identity_at(&fresh, password, path)?;
    Ok(fresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "vox-identity-test-{}-{}.key.enc",
            name,
            std::process::id()
        ));
        let _ = fs::remove_file(&p);
        p
    }

    #[test]
    fn rotate_creates_identity_when_absent() {
        let path = tmp_path("absent");
        assert!(!path.exists());
        let id = rotate_identity_at("pw-correct", &path).expect("rotate should create");
        assert!(path.exists());
        // The freshly-saved identity must load back with the same password.
        let loaded = load_identity_at("pw-correct", &path).expect("load back");
        assert_eq!(loaded.node_id(), id.node_id());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rotate_changes_node_id() {
        let path = tmp_path("changes");
        let first = rotate_identity_at("pw", &path).expect("first");
        let second = rotate_identity_at("pw", &path).expect("second");
        assert_ne!(
            first.node_id(),
            second.node_id(),
            "rotation must yield a new keypair"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rotate_refuses_wrong_password() {
        let path = tmp_path("wrongpw");
        rotate_identity_at("correct", &path).expect("seed");
        let err = rotate_identity_at("incorrect", &path).expect_err("must refuse");
        assert!(err.to_string().contains("Incorrect master password"));
        // The original identity must be intact and still unlock with the real pw.
        assert!(load_identity_at("correct", &path).is_ok());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rotate_rejects_empty_password() {
        let path = tmp_path("emptypw");
        assert!(rotate_identity_at("", &path).is_err());
        let _ = fs::remove_file(&path);
    }
}
