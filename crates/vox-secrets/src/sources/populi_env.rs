use secrecy::SecretString;

use crate::errors::SecretError;
use crate::types::SecretSource;

// vox:defactored-from vox-config 2026-09-05
// `vox_config::paths::find_repo_root` is the canonical implementation of this walk-up
// (see its doc comment on why current_dir().join(".vox") is wrong: it mints a stray
// `.vox/` tree wherever the shell happens to be, and a stray tree under `crates/` has
// broken cargo outright because the root manifest globs `crates/*`). vox-secrets cannot
// take a dependency on vox-config here: vox-config already depends on vox-secrets (see
// crates/vox-config/Cargo.toml), so the reverse edge would be a cycle, and vox-secrets is
// layer 1 to vox-config's layer 2 (downward-only dependency rule). The walked logic is
// under 50 lines, so per the workspace's defactor policy it is duplicated here rather
// than restructuring either crate's layer assignment.
fn find_repo_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join(".git").exists() || d.join("Vox.toml").is_file() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

fn candidate_mesh_env_paths() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for root_var in ["VOX_WORKSPACE_ROOT", "VOX_REPO_ROOT"] {
        if let Ok(root) = std::env::var(root_var)
            && !root.trim().is_empty()
        {
            out.push(
                std::path::PathBuf::from(root.trim())
                    .join(".vox")
                    .join("populi")
                    .join("mesh.env"),
            );
        }
    }
    // Anchored to the repo root (walking up for `.git`/`Vox.toml`), never to a bare
    // `current_dir()` — see the `find_repo_root` doc comment above. When cwd isn't
    // inside a repository there is nothing to anchor to, so no candidate is pushed
    // here; the `auth_json::vox_dir()` (home) candidate below still covers that case.
    if let Ok(cwd) = std::env::current_dir()
        && let Some(repo_root) = find_repo_root(&cwd)
    {
        out.push(repo_root.join(".vox").join("populi").join("mesh.env"));
    }
    out.push(
        crate::sources::auth_json::vox_dir()
            .join("populi")
            .join("mesh.env"),
    );
    out
}

/// Read `KEY=value` from the first `.vox/populi/mesh.env` candidate that contains `canonical_key`.
#[must_use]
pub fn read_populi_env_key(canonical_key: &str) -> Option<(SecretString, SecretSource)> {
    let needle = canonical_key.trim();
    if needle.is_empty() {
        return None;
    }
    for path in candidate_mesh_env_paths() {
        let raw = match vox_bounded_fs::read_utf8_path_capped(&path)
            .map_err(|e| SecretError::Io(e.to_string()))
        {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        for line in raw.lines() {
            let t = line.trim();
            if t.starts_with('#') || t.is_empty() {
                continue;
            }
            let Some((key, value)) = t.split_once('=') else {
                continue;
            };
            if key.trim() != needle {
                continue;
            }
            let v = value.trim().to_string();
            if v.is_empty() {
                return None;
            }
            return Some((
                SecretString::new(v.into_boxed_str()),
                SecretSource::PopuliEnv,
            ));
        }
    }
    None
}

#[must_use]
pub fn read_mesh_token_from_populi_env() -> Option<(SecretString, SecretSource)> {
    read_populi_env_key("VOX_MESH_TOKEN")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_repo_root_anchors_to_the_repo_root_not_a_deep_subdirectory() {
        // Guards the bug fixed in candidate_mesh_env_paths(): it used to build
        // `current_dir().join(".vox")`, which mints a stray `.vox/` wherever the
        // shell happens to be when cwd is a deep subdirectory of the repo, rather
        // than anchoring to the repo root. Mirrors
        // `vox_config::paths::repo_dot_vox_dir_anchors_to_the_repo_root_not_the_cwd`.
        let tmp = std::env::temp_dir().join(format!(
            "vox-populi-env-test-{}-{}",
            std::process::id(),
            line!()
        ));
        let deep = tmp.join("crates").join("some-crate").join("src");
        std::fs::create_dir_all(&deep).expect("mkdir deep");
        std::fs::create_dir_all(tmp.join(".git")).expect("mkdir .git");

        let found = find_repo_root(&deep).expect("repo root must be found from a deep subdir");
        assert_eq!(
            std::fs::canonicalize(&found).unwrap(),
            std::fs::canonicalize(&tmp).unwrap(),
            "walking up from a subdirectory must find the repo root, not the subdirectory"
        );

        let candidate = found.join(".vox").join("populi").join("mesh.env");
        assert!(
            candidate.starts_with(&tmp),
            "candidate must be anchored under the repo root: {candidate:?}"
        );
        assert!(
            !candidate.starts_with(&deep),
            "candidate must not be anchored under the deep subdirectory: {candidate:?}"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn find_repo_root_returns_none_outside_any_repository() {
        let tmp = std::env::temp_dir().join(format!(
            "vox-populi-env-test-no-repo-{}-{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&tmp).expect("mkdir");

        // No `.git` or `Vox.toml` anywhere up from a lone temp dir under the OS temp
        // root (which itself carries neither marker).
        assert_eq!(
            find_repo_root(&tmp),
            None,
            "a directory with no .git/Vox.toml ancestor must not resolve to a repo root"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }
}
