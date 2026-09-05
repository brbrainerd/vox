//! Cross-platform path and directory resolution.
//!
//! Single source of truth for VOX_DATA_DIR, VOX_USER_ID, and platform data dirs.
//! Precedence: env vars > platform defaults.

use std::path::{Path, PathBuf};

/// Application directory name under the base data dir.
pub const APP_DIR_NAME: &str = "vox";
/// Default database filename.
pub const DEFAULT_DB_FILENAME: &str = "vox.db";

/// Resolve the Vox data directory. Env `VOX_DATA_DIR` overrides; else platform default.
pub fn data_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("VOX_DATA_DIR")
        && !dir.is_empty()
    {
        let path = PathBuf::from(dir);
        std::fs::create_dir_all(&path).ok();
        return Some(path);
    }
    let base = platform_data_dir()?;
    let path = base.join(APP_DIR_NAME);
    std::fs::create_dir_all(&path).ok();
    Some(path)
}

/// Default database path: `<data_dir>/vox.db`.
pub fn default_db_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join(DEFAULT_DB_FILENAME))
}

/// State directory for durable objects: `<data_dir>/state/`.
pub fn state_dir() -> Option<PathBuf> {
    data_dir().map(|d| {
        let p = d.join("state");
        std::fs::create_dir_all(&p).ok();
        p
    })
}

/// Config directory: `<data_dir>/config/`.
pub fn config_dir() -> Option<PathBuf> {
    data_dir().map(|d| {
        let p = d.join("config");
        std::fs::create_dir_all(&p).ok();
        p
    })
}

/// Skill discovery roots, highest precedence first.
///
/// `.vox/skills` is Vox-native; `.cursor/skills` is the Cursor IDE convention;
/// `.agents/skills` is the vendor-neutral
/// [agentskills.io](https://agentskills.io) convention (Codex, Cursor, Copilot,
/// Amp); `.claude/skills` is the most widely honored compatibility path.
/// Workspace beats user-home. On id collision, callers install first-root-wins.
///
/// `assets/skills` (last) holds Apache-2.0 vendored skills shipped with the
/// Vox source tree. It is shadowed by every interop root so workspace or user
/// skills always win.
pub fn skill_search_roots(workspace_root: &Path) -> Vec<PathBuf> {
    const SUBDIRS: [&str; 4] = [
        REPO_SKILLS_DIR,
        ".cursor/skills",
        ".agents/skills",
        ".claude/skills",
    ];
    let mut roots: Vec<PathBuf> = SUBDIRS.iter().map(|d| workspace_root.join(d)).collect();
    if let Some(home) = dirs::home_dir() {
        roots.extend(SUBDIRS.iter().map(|d| home.join(d)));
    }
    // Lowest precedence: vendored Apache-2.0 skills bundled with the repo.
    roots.push(workspace_root.join("assets/skills"));
    roots
}

/// Current user id for local usage. Env `VOX_USER_ID` or platform username or `"local-user"`.
pub fn local_user_id() -> String {
    if let Ok(id) = std::env::var("VOX_USER_ID")
        && !id.is_empty()
    {
        return id;
    }
    #[cfg(target_os = "windows")]
    if let Ok(user) = std::env::var("USERNAME")
        && !user.is_empty()
    {
        return user;
    }
    #[cfg(not(target_os = "windows"))]
    if let Ok(user) = std::env::var("USER")
        && !user.is_empty()
    {
        return user;
    }
    "local-user".to_string()
}

fn platform_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    return std::env::var("APPDATA").ok().map(PathBuf::from);

    #[cfg(target_os = "macos")]
    return Some(user_home_dir().join("Library").join("Application Support"));

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
            && !xdg.is_empty()
        {
            return Some(PathBuf::from(xdg));
        }
        Some(user_home_dir().join(".local").join("share"))
    }
}

/// Best-effort user home (`HOME`, `USERPROFILE`, or `HOMEDRIVE`+`HOMEPATH`; else `.`).
pub fn user_home_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(h) = std::env::var("USERPROFILE")
            && !h.is_empty()
        {
            return PathBuf::from(h);
        }
        if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
            let p = format!("{drive}{path}");
            if !p.is_empty() {
                return PathBuf::from(p);
            }
        }
        PathBuf::from(".")
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// `~/.vox` under [`user_home_dir`] (CLI script cache, etc.).
pub fn dot_vox_user_dir() -> PathBuf {
    user_home_dir().join(".vox")
}

/// The repository root containing `cwd`, found by walking up for `.git` or
/// `Vox.toml`. `None` when `cwd` is not inside a repository.
///
/// Exists so nothing has to reach for `current_dir()` when it means "this repo".
pub fn find_repo_root(start: &std::path::Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join(".git").exists() || d.join("Vox.toml").is_file() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

/// The `.vox` directory for repo-scoped state, anchored to the **repository
/// root** — never to the current working directory.
///
/// WHY THIS EXISTS. Several call sites used to build
/// `current_dir().join(".vox")`, so every `vox` invocation from a subdirectory
/// minted a fresh `.vox/` wherever the shell happened to be. Telemetry
/// initializes before command dispatch, so this fired on *every* command, and
/// two stray trees were found in one afternoon — one under
/// `crates/vox-cli/src/commands/diagnostics/doctor/`, one under
/// `docs/superpowers/plans/`. A stray `.vox` under `crates/` has previously
/// broken cargo outright, because the root manifest globs `crates/*`.
///
/// Falls back to `~/.vox` when not inside a repository, which is the correct
/// home for a user-scoped tool run from an arbitrary directory. It never
/// returns a bare-CWD path.
pub fn repo_dot_vox_dir() -> PathBuf {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| find_repo_root(&cwd))
        .map(|root| root.join(".vox"))
        .unwrap_or_else(dot_vox_user_dir)
}

/// Script compilation cache under `~/.vox/script-cache` or `~/.vox/script-cache-wasi`.
pub fn script_cache_dir(wasi_target: bool) -> PathBuf {
    let name = if wasi_target {
        "script-cache-wasi"
    } else {
        "script-cache"
    };
    dot_vox_user_dir().join(name)
}

/// `.vox/cache/repos/<repository_id>` under a repository root (MCP index, orchestrator cache).
pub fn repo_tooling_cache_dir(repo_root: &Path, repository_id: &str) -> PathBuf {
    repo_root
        .join(".vox")
        .join("cache")
        .join("repos")
        .join(repository_id)
}

/// Memory shard directory under [`repo_tooling_cache_dir`].
pub fn repo_memory_cache_dir(repo_root: &Path, repository_id: &str) -> PathBuf {
    repo_tooling_cache_dir(repo_root, repository_id).join("memory")
}

/// Portable backend artifact lane metadata under `<repo_root>/.vox/backend-artifact/`
/// (SBOM and signing attestations before OCI promotion; see portability SSOT).
#[must_use]
pub fn repo_backend_artifact_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".vox").join("backend-artifact")
}

/// SCIENTIA research mesh intake (orchestrator research broadcast → publisher scholarly pipeline).
///
/// Under `<repo_root>/.vox/scientia/research-mesh-intake/` — see `vox-publisher::research_mesh`.
#[must_use]
pub fn repo_scientia_research_mesh_intake_dir(repo_root: &Path) -> PathBuf {
    repo_root
        .join(".vox")
        .join("scientia")
        .join("research-mesh-intake")
}

/// Promoted SCIENTIA mesh ledger (JSONL) under `<repo_root>/.vox/scientia/research-mesh-promoted/`.
#[must_use]
pub fn repo_scientia_research_mesh_promoted_dir(repo_root: &Path) -> PathBuf {
    repo_root
        .join(".vox")
        .join("scientia")
        .join("research-mesh-promoted")
}

/// Basename for MCP session dirs (`.vox/sessions/<repository_id>` under repo root).
pub const MCP_SESSIONS_DIR_BASENAME: &str = ".vox/sessions";

// ─── repo-relative path string constants (consumed by globbers / config loaders) ───
//
// These are *raw path strings* — most callers should compose them via `PathBuf::join`,
// but globbers, manifest readers, and config keys need them as `&str`. Keeping them
// in one place lets `drift/vox-path-literal` flag stragglers.

/// `.vox/` repo subdirectory (root of all repo-scoped Vox state).
pub const REPO_DOT_VOX_DIR: &str = ".vox";
/// `.vox-cache` repo subdirectory (compiler / package cache).
pub const REPO_DOT_VOX_CACHE_DIR: &str = ".vox-cache";
/// `.vox/agents` glob — agent scope discovery.
pub const REPO_AGENTS_DIR: &str = ".vox/agents";
/// `.vox/agents/**` glob — recursive agent discovery.
pub const REPO_AGENTS_GLOB: &str = ".vox/agents/**";
/// `.vox/cache/` repo subdirectory.
pub const REPO_CACHE_DIR: &str = ".vox/cache";
/// `.vox/cache/graphify/<corpus_id>` — Tier D graphify map cache (see graphify SSOT).
pub const REPO_GRAPHIFY_CACHE_SUBDIR: &str = "graphify";
/// `.vox/cache/` prefix (with trailing slash; for ignore-list prefix matching).
pub const REPO_CACHE_DIR_PREFIX: &str = ".vox/cache/";
/// `.vox/cache/drift` — drift-check cache root.
pub const REPO_DRIFT_CACHE_DIR: &str = ".vox/cache/drift";
/// `.vox/cache/drift/baseline.json` — drift-check baseline snapshot.
pub const REPO_DRIFT_BASELINE_FILE: &str = ".vox/cache/drift/baseline.json";
/// `.vox/models` directory — repo-local ML model fallback root.
pub const REPO_MODELS_DIR: &str = ".vox/models";
/// `.vox/memory` directory.
pub const REPO_MEMORY_DIR: &str = ".vox/memory";
/// `.vox/MEMORY.md` user-edited memory index.
pub const REPO_MEMORY_INDEX_FILE: &str = ".vox/MEMORY.md";
/// `.vox/ludus` ludus pack directory.
pub const REPO_LUDUS_DIR: &str = ".vox/ludus";
/// `.vox/ludus/lex-pack.toml` ludus manifest.
pub const REPO_LUDUS_PACK_FILE: &str = ".vox/ludus/lex-pack.toml";
/// `.vox/speech_lexicon.json` oratio lexicon override.
pub const REPO_SPEECH_LEXICON_FILE: &str = ".vox/speech_lexicon.json";
/// `.vox/toolchain-upgrade-rollback.json` repo upgrade rollback snapshot.
pub const REPO_TOOLCHAIN_ROLLBACK_FILE: &str = ".vox/toolchain-upgrade-rollback.json";
/// Project-level VOX.md memory file (agent memory for this repository).
pub const REPO_VOX_MD_FILE: &str = ".vox/VOX.md";

/// `.vox/skills` — repo-local Vox skill discovery root (highest precedence in skill search).
pub const REPO_SKILLS_DIR: &str = ".vox/skills";
/// `.vox/cache/vox-graph` — legacy Vox code-graph cache root (pre-graphify migration).
pub const REPO_VOX_GRAPH_CACHE_DIR: &str = ".vox/cache/vox-graph";
/// `.vox/cache/vox-graph/registered.v1.json` — Vox graph corpus registry overlay.
pub const REPO_VOX_GRAPH_REGISTERED_FILE: &str = ".vox/cache/vox-graph/registered.v1.json";
/// `.vox/cache/graphify/repo-code-graph` — Graphify repository code-graph corpus directory.
pub const REPO_GRAPHIFY_REPO_CODE_GRAPH_DIR: &str = ".vox/cache/graphify/repo-code-graph";
/// `.vox/cache/graphify/registered.v1.json` — Graphify legacy corpus registry overlay.
pub const REPO_GRAPHIFY_REGISTERED_FILE: &str = ".vox/cache/graphify/registered.v1.json";
/// `.vox/cache/graphify/ext` — Graphify external-source corpus cache directory.
pub const REPO_GRAPHIFY_EXT_DIR: &str = ".vox/cache/graphify/ext";
/// `.vox/cache/graphify/ext/graph.json` — Graphify external-source graph file.
pub const REPO_GRAPHIFY_EXT_GRAPH_FILE: &str = ".vox/cache/graphify/ext/graph.json";
/// `.vox/cache/graphify/ext/.graphify_manifest.v1.json` — Graphify external-source manifest.
pub const REPO_GRAPHIFY_EXT_MANIFEST_FILE: &str =
    ".vox/cache/graphify/ext/.graphify_manifest.v1.json";
/// `.vox/cache/graphify/vox-gui-surface` — Graphify GUI surface corpus cache directory.
pub const REPO_GRAPHIFY_GUI_SURFACE_DIR: &str = ".vox/cache/graphify/vox-gui-surface";
/// `.vox/cache/graphify-src` — Graphify source-scan cache directory.
pub const REPO_GRAPHIFY_SRC_CACHE_DIR: &str = ".vox/cache/graphify-src";
/// `.vox/corpus/heal_pairs.jsonl` — MENS heal-pair training corpus (repo-local).
pub const REPO_CORPUS_HEAL_PAIRS_FILE: &str = ".vox/corpus/heal_pairs.jsonl";
/// `.vox/db/vox.db` — repo-scoped database path (distinct from the user data-dir DB).
pub const REPO_DB_PATH: &str = ".vox/db/vox.db";

/// MCP session persistence: `.vox/sessions/<repository_id>` (relative to repository root).
pub fn mcp_sessions_dir(repository_id: &str) -> PathBuf {
    PathBuf::from(MCP_SESSIONS_DIR_BASENAME).join(repository_id)
}

#[cfg(test)]
mod repo_path_tests {

    #[test]
    fn repo_dot_vox_dir_anchors_to_the_repo_root_not_the_cwd() {
        // Guards the bug that produced two stray .vox trees in one afternoon:
        // `current_dir().join(".vox")` minted one wherever the shell happened to
        // be, and telemetry runs before command dispatch, so it fired on every
        // invocation. A stray .vox under crates/ has broken cargo before, because
        // the root manifest globs crates/*.
        let tmp = std::env::temp_dir().join(format!("voxroot-{}", std::process::id()));
        let deep = tmp.join("crates").join("some-crate").join("src");
        std::fs::create_dir_all(&deep).expect("mkdir");
        std::fs::create_dir_all(tmp.join(".git")).expect("mkdir .git");

        let found = find_repo_root(&deep).expect("repo root must be found from a deep subdir");
        assert_eq!(
            std::fs::canonicalize(&found).unwrap(),
            std::fs::canonicalize(&tmp).unwrap(),
            "walking up from a subdirectory must find the repo root, not the subdirectory"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn repo_dot_vox_dir_never_returns_a_bare_cwd_path() {
        // The fallback when outside a repository must be ~/.vox, never ".".
        let got = repo_dot_vox_dir();
        assert!(
            got.is_absolute(),
            "repo_dot_vox_dir must be absolute, got {got:?}"
        );
        assert_ne!(
            got,
            std::path::PathBuf::from(".").join(".vox"),
            "repo_dot_vox_dir must never fall back to a bare CWD path"
        );
    }
    use super::*;

    #[test]
    fn scientia_research_mesh_intake_is_under_dot_vox() {
        let p = repo_scientia_research_mesh_intake_dir(Path::new("/workspace/repo"));
        let s = p.to_string_lossy();
        assert!(
            s.contains(".vox") && s.contains("scientia") && s.contains("research-mesh-intake"),
            "{s}"
        );
    }

    #[test]
    fn scientia_research_mesh_promoted_is_under_dot_vox() {
        let p = repo_scientia_research_mesh_promoted_dir(Path::new("/workspace/repo"));
        let s = p.to_string_lossy();
        assert!(
            s.contains(".vox") && s.contains("scientia") && s.contains("research-mesh-promoted"),
            "{s}"
        );
    }

    #[test]
    fn skill_search_roots_orders_vox_then_agents_then_claude() {
        let ws = Path::new("/repo");
        let roots = skill_search_roots(ws);
        // Workspace roots come first (highest precedence), in canonical order.
        let rel: Vec<String> = roots
            .iter()
            .take(4)
            .map(|p| {
                p.strip_prefix(ws)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(
            rel,
            vec![
                ".vox/skills",
                ".cursor/skills",
                ".agents/skills",
                ".claude/skills",
            ],
        );
        // assets/skills is always the last (lowest-precedence) entry.
        assert_eq!(
            roots.last().unwrap().to_string_lossy().replace('\\', "/"),
            "/repo/assets/skills"
        );
        // User-home roots mirror the same order under the home dir, when present.
        if let Some(home) = dirs::home_dir() {
            assert_eq!(roots.len(), 9);
            assert_eq!(roots[4], home.join(".vox/skills"));
            assert_eq!(roots[5], home.join(".cursor/skills"));
            assert_eq!(roots[6], home.join(".agents/skills"));
            assert_eq!(roots[7], home.join(".claude/skills"));
        } else {
            assert_eq!(roots.len(), 5);
        }
    }
}
