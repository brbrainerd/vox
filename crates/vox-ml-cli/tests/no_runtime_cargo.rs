//! Behavioral guard for spec §4.1 P6 ("no runtime cargo"): the CUDA plugin
//! auto-heal path must never shell out to a compiler.
//!
//! A bare `grep Command::new("cargo")` is defeatable (`Command::new(cargo_bin())`,
//! `env cargo`, a `const CARGO: &str`), so this asserts behaviorally instead:
//! point `PATH` at a directory containing only a fake `vox` script and confirm
//! the heal path still succeeds — which is only possible if it never tried to
//! spawn `cargo`, `cc`, `nvcc`, or `cl.exe` (none of which are reachable on
//! this restricted `PATH`; any attempt would fail with "program not found").
//!
//! This crate only exposes the `mens` command tree behind the `gpu` feature
//! (see `crates/vox-ml-cli/src/commands/mens/plugin_heal.rs`'s
//! `#![cfg(feature = "gpu")]`), so this whole test file is a no-op without it.
#![cfg(feature = "gpu")]

/// With `PATH` restricted to a directory holding only a fake `vox`, auto-heal
/// must still be able to "reinstall" the plugin by shelling out to `vox
/// plugin install <id> --yes` — never to `cargo build`.
#[test]
fn auto_heal_reaches_post_heal_probe_without_a_compiler_on_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log_path = dir.path().join("invocations.log");
    write_fake_vox(dir.path(), &log_path);

    let original_path = std::env::var_os("PATH");
    // SAFETY: this is the only test in this binary that touches `PATH`, and
    // integration test files each compile to their own test binary, so there
    // is no cross-test race on the process environment.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("PATH", dir.path());
    }

    let result = vox_ml_cli::commands::mens::plugin_heal::ensure_cuda_plugin(true);

    // SAFETY: see the comment on the same call above — this restores the
    // pre-test PATH before any other test in this binary can observe it.
    #[allow(unsafe_code)]
    unsafe {
        match &original_path {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
    }

    // The plugin genuinely isn't installed in this test environment, so even
    // a successful `vox plugin install` (the fake script always exits 0)
    // leaves the post-heal probe unable to load it — `ensure_cuda_plugin`
    // correctly reports that as "still unusable". What distinguishes the
    // fixed code from the old `cargo build` path is *how far it got*: the
    // fake `vox` must have been invoked at all, which only happens if the
    // heal path never tried (and failed on) `cargo`/`cc`/`nvcc`/`cl.exe`
    // first — none of which exist on the restricted `PATH` above.
    let err = result.expect_err("plugin genuinely isn't installed; heal must still fail overall");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("still unusable"),
        "expected the error to come from the *post-heal* probe (proving the \
         fake `vox` ran successfully), not from a failed compiler spawn: {msg}"
    );

    let log = std::fs::read_to_string(&log_path)
        .expect("fake `vox` must have been invoked and logged its argv");
    assert_eq!(
        log.trim(),
        "plugin install mens-candle-cuda --yes",
        "auto-heal must invoke exactly `vox plugin install <id> --yes`"
    );
}

#[cfg(unix)]
fn write_fake_vox(dir: &std::path::Path, log_path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let script = dir.join("vox");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"$@\" >> \"{}\"\nexit 0\n",
            log_path.display()
        ),
    )
    .expect("writing fake vox script");
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).expect("chmod +x fake vox script");
}

#[cfg(windows)]
fn write_fake_vox(dir: &std::path::Path, log_path: &std::path::Path) {
    use std::io::Write as _;

    let script = dir.join("vox.bat");
    let mut f = std::fs::File::create(&script).expect("creating fake vox.bat");
    writeln!(f, "@echo off").unwrap();
    writeln!(f, "echo %* >> \"{}\"", log_path.display()).unwrap();
    writeln!(f, "exit /b 0").unwrap();
}
