//! Cargo binary + worktree resolution.
//!
//! The shim must (a) decide whether a subcommand is worth queueing, (b) find the
//! worktree root that owns the target lock, and (c) resolve the *real* cargo
//! (the rustup proxy) without recursing into itself.

use std::path::{Path, PathBuf};

/// Subcommands whose runs take the cargo target lock and are worth queueing.
pub fn is_build_subcommand(sub: &str) -> bool {
    matches!(sub, "build" | "test" | "check" | "clippy" | "run" | "bench")
}

/// Walk up from `start` to the first directory that is a vox worktree root,
/// identified by having BOTH `.cargo/config.toml` and `Cargo.toml`.
///
/// Requiring `Cargo.toml` is essential: the global `~/.cargo/config.toml` exists
/// for every user, so matching on `.cargo/config.toml` alone would treat the
/// entire home directory as a worktree. A real cargo workspace root always has a
/// `Cargo.toml` next to its `.cargo/config.toml`.
///
/// Returns `None` if no such ancestor exists, in which case the caller bypasses
/// the queue and runs cargo directly.
pub fn worktree_root_of(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| d.join(".cargo/config.toml").is_file() && d.join("Cargo.toml").is_file())
        .map(Path::to_path_buf)
}

/// The rustup cargo proxy under `$CARGO_HOME`/`~/.cargo/bin`, if present. This is
/// never a shim copy and honours `rust-toolchain.toml` / `+toolchain`, so the
/// shim prefers it over a PATH scan. Mirrors the repo's own `cargo_bin()`.
pub fn cargo_home_proxy() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "cargo.exe" } else { "cargo" };
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".cargo")))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))?;
    let p = home.join("bin").join(exe);
    p.is_file().then_some(p)
}

/// Resolve the real cargo by scanning `path_var` for the first `cargo` that is
/// neither the shim itself nor a sibling copy living in the shim's own directory
/// (e.g. `target/debug/cargo.exe`, `target/debug/deps/cargo.exe` produced during
/// `cargo test`). Skipping same-directory siblings is essential: matching only
/// the exact `own_exe` path let a sibling shim copy be picked as "real cargo",
/// causing infinite recursion (a fork bomb).
///
/// Returns `None` if our own identity cannot be established (we must not guess).
pub fn resolve_real_cargo(path_var: &str, own_exe: &Path) -> Option<PathBuf> {
    let own = own_exe.canonicalize().ok()?;
    let own_dir = own.parent().map(Path::to_path_buf);
    let exe = if cfg!(windows) { "cargo.exe" } else { "cargo" };
    for dir in std::env::split_paths(path_var) {
        let cand = dir.join(exe);
        if !cand.is_file() {
            continue;
        }
        let canon = match cand.canonicalize() {
            Ok(c) => c,
            Err(_) => continue,
        };
        if canon == own {
            continue; // ourselves
        }
        if canon.parent().map(Path::to_path_buf) == own_dir {
            continue; // a sibling copy in our own directory — also a shim
        }
        return Some(canon);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_subcommands_classified() {
        assert!(is_build_subcommand("test"));
        assert!(is_build_subcommand("clippy"));
        assert!(!is_build_subcommand("fmt"));
        assert!(!is_build_subcommand("add"));
    }

    #[test]
    fn worktree_root_found_via_cargo_config() {
        let tmp = tempfile::tempdir().unwrap();
        let wt = tmp.path().join("wt");
        std::fs::create_dir_all(wt.join(".cargo")).unwrap();
        std::fs::write(wt.join(".cargo/config.toml"), "[env]\n").unwrap();
        std::fs::write(wt.join("Cargo.toml"), "[workspace]\n").unwrap();
        let deep = wt.join("crates/foo/src");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(
            worktree_root_of(&deep).unwrap().canonicalize().unwrap(),
            wt.canonicalize().unwrap()
        );
    }

    #[test]
    fn worktree_root_none_outside() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(worktree_root_of(tmp.path()).is_none());
    }

    #[test]
    fn resolve_skips_self() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = if cfg!(windows) { "cargo.exe" } else { "cargo" };
        let shim_dir = tmp.path().join("shim");
        let real_dir = tmp.path().join("real");
        std::fs::create_dir_all(&shim_dir).unwrap();
        std::fs::create_dir_all(&real_dir).unwrap();
        let shim = shim_dir.join(exe);
        let real = real_dir.join(exe);
        std::fs::write(&shim, b"x").unwrap();
        std::fs::write(&real, b"y").unwrap();
        let path = std::env::join_paths([&shim_dir, &real_dir]).unwrap();
        let got = resolve_real_cargo(path.to_str().unwrap(), &shim).unwrap();
        assert_eq!(got.canonicalize().unwrap(), real.canonicalize().unwrap());
    }

    #[test]
    fn resolve_skips_same_dir_sibling_copy() {
        // A shim copy sharing the shim's own directory (as `cargo test` produces
        // in target/debug) must NOT be chosen — that would fork-bomb.
        let tmp = tempfile::tempdir().unwrap();
        let exe = if cfg!(windows) { "cargo.exe" } else { "cargo" };
        let own_dir = tmp.path().join("debug");
        let real_dir = tmp.path().join("cargohome");
        std::fs::create_dir_all(&own_dir).unwrap();
        std::fs::create_dir_all(&real_dir).unwrap();
        // The shim runs as own.exe; a *different-named* sibling copy is irrelevant,
        // but a same-name `cargo.exe` earlier on PATH in the same dir is the trap.
        let own = own_dir.join("vox-cargo.exe");
        std::fs::write(&own, b"self").unwrap();
        let sibling = own_dir.join(exe); // same dir as own -> must be skipped
        std::fs::write(&sibling, b"sibling-shim").unwrap();
        let real = real_dir.join(exe);
        std::fs::write(&real, b"real").unwrap();
        let path = std::env::join_paths([&own_dir, &real_dir]).unwrap();
        let got = resolve_real_cargo(path.to_str().unwrap(), &own).unwrap();
        assert_eq!(got.canonicalize().unwrap(), real.canonicalize().unwrap());
    }
}
