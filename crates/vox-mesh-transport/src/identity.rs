//! Persisted mesh identity. The `EndpointId` IS the ed25519 public key.
//!
//! Deliberately separate from `vox_identity`'s node identity: that one is
//! password-sealed and may be locked, which must never prevent the mesh from
//! starting headless. The two keys cannot share a Rust type anyway —
//! `vox-crypto` pins ed25519-dalek 2.x and iroh resolves 3.0.0 — but both
//! round-trip through 32 raw bytes, so one stored seed could derive both.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use iroh::SecretKey;

/// Magic + version prefix. Without it, a 32-byte file of garbage is
/// indistinguishable from a key: every 32-byte string is a valid ed25519 seed,
/// so length alone cannot detect corruption. The version byte is also where a
/// future DPAPI- or Keychain-wrapped payload announces itself.
const MAGIC: &[u8; 16] = b"vox-mesh-key-v1\n";

/// Total on-disk size: [`MAGIC`] followed by the raw 32-byte seed.
const FILE_LEN: usize = MAGIC.len() + 32;

/// Load the mesh identity at `path`, generating and persisting one if absent.
///
/// Never silently regenerates: a key that is present but unreadable is an
/// error, because replacing it would orphan every peer that trusted the old
/// public key and the user would see only "pairing stopped working".
pub fn load_or_create(path: &Path) -> Result<SecretKey> {
    match std::fs::read(path) {
        Ok(bytes) => {
            check_permissions(path)?;
            parse(&bytes)
                .with_context(|| format!("mesh identity at {} is unreadable", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let sk = SecretKey::generate();
            write_new(path, &sk)?;
            Ok(sk)
        }
        Err(e) => {
            Err(anyhow::Error::new(e)
                .context(format!("reading mesh identity at {}", path.display())))
        }
    }
}

fn parse(bytes: &[u8]) -> Result<SecretKey> {
    if bytes.len() != FILE_LEN {
        bail!(
            "expected {FILE_LEN} bytes ({} magic + 32 seed), found {}",
            MAGIC.len(),
            bytes.len()
        );
    }
    if &bytes[..MAGIC.len()] != MAGIC {
        bail!("bad magic — this file was not written by vox-mesh-transport");
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&bytes[MAGIC.len()..]);
    Ok(SecretKey::from_bytes(&seed))
}

/// Write a freshly generated key, creating the parent directory if needed.
///
/// Written to a sibling temp file and renamed, so a crash mid-write leaves
/// either the old key or none — never a truncated one that fails to parse and
/// takes the mesh down with an error nobody connects to the reboot.
fn write_new(path: &Path, sk: &SecretKey) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let mut buf = Vec::with_capacity(FILE_LEN);
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&sk.to_bytes());

    let tmp = tmp_path(path);
    write_private(&tmp, &buf)
        .with_context(|| format!("writing mesh identity to {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} to {}", tmp.display(), path.display()))?;
    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Create the file with owner-only permissions **at creation time**.
///
/// Writing first and chmod-ing after would leave a window in which the private
/// key is world-readable on a shared machine.
#[cfg(unix)]
fn write_private(path: &Path, buf: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(buf)?;
    f.sync_all()
}

/// Windows leaves default ACLs here.
///
/// The honest statement today is that the key is protected by the profile
/// directory's ACL and nothing more. DPAPI (`CryptProtectData`, user scope)
/// is the intended hardening — at-rest encryption with no password prompt, so
/// headless start is preserved — and it is deliberately not faked here.
// TODO(mesh-phase1): wrap the seed with DPAPI on Windows and the Keychain on
// macOS; the MAGIC version byte exists so the format can say which.
#[cfg(not(unix))]
fn write_private(path: &Path, buf: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, buf)
}

/// Refuse a key that anyone but the owner can read, and name the fix.
///
/// Loading it anyway would teach the user nothing and leave a private key
/// readable by every account on the machine.
#[cfg(unix)]
fn check_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let mode = std::fs::metadata(path)
        .with_context(|| format!("stat {}", path.display()))?
        .permissions()
        .mode();
    if mode & 0o077 != 0 {
        bail!(
            "mesh identity {} is group/world readable (mode {:o}); run: chmod 600 {}",
            path.display(),
            mode & 0o777,
            path.display()
        );
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id_of(sk: &SecretKey) -> iroh::EndpointId {
        sk.public()
    }

    #[test]
    fn a_generated_key_is_stable_across_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        assert_eq!(
            id_of(&load_or_create(&p).unwrap()),
            id_of(&load_or_create(&p).unwrap()),
            "identity must survive restart or every pairing breaks"
        );
    }

    #[test]
    fn a_corrupt_key_file_is_an_error_not_a_silent_new_identity() {
        // Silently regenerating would orphan every peer that trusted the old key.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        std::fs::write(&p, b"not a key").unwrap();
        assert!(load_or_create(&p).is_err());
    }

    #[test]
    fn a_right_sized_file_with_the_wrong_magic_is_still_an_error() {
        // Length alone cannot detect corruption: every 32-byte string is a
        // valid ed25519 seed, so without the magic this file would load as a
        // brand-new identity and orphan every peer.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        std::fs::write(&p, [0xAAu8; FILE_LEN]).unwrap();
        // fs::write creates 0644, which trips the permission check first; this
        // test is about the magic, so satisfy the earlier gate deliberately.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let e = format!("{:#}", load_or_create(&p).unwrap_err());
        assert!(e.contains("bad magic"), "{e}");
    }

    #[test]
    fn a_missing_parent_directory_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("nested").join("deeper").join("mesh.key");
        assert!(load_or_create(&p).is_ok());
        assert!(p.exists());
    }

    #[test]
    fn no_temp_file_survives_a_successful_write() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        load_or_create(&p).unwrap();
        assert!(
            !tmp_path(&p).exists(),
            "temp file must be renamed, not left behind"
        );
    }

    #[test]
    #[cfg(unix)]
    fn the_key_file_is_not_readable_by_group_or_other() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        load_or_create(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o077,
            0,
            "a plaintext private key must not be group/world readable"
        );
    }

    #[test]
    #[cfg(unix)]
    fn a_world_readable_key_is_refused_with_the_fix_in_the_message() {
        // Loading silently would teach the user nothing.
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("mesh.key");
        load_or_create(&p).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        let e = load_or_create(&p).unwrap_err().to_string();
        assert!(e.contains("chmod 600"), "error must name the fix: {e}");
    }
}
