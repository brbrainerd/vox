//! Smoke: `VoxCliRoot` parses global flags + `completions` / Latin groupings.
//!
//! **Windows:** default test-thread stacks (~1 MiB) overflow when clap builds/parses the full
//! `VoxCliRoot` tree. Run heavy parse/help work on a dedicated thread with a larger stack.

use clap::CommandFactory;
use clap::Parser;
use vox_cli::commands::ci::CiCmd;
use vox_cli::commands::secrets::SecretsCmd;
use vox_cli::{Cli, VoxCliRoot};

#[cfg(windows)]
fn with_cli_parse_stack<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    const STACK: usize = 8 * 1024 * 1024;
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(f)
        .expect("spawn cli parse stack thread")
        .join()
        .expect("cli parse stack thread panicked")
}

#[cfg(not(windows))]
fn with_cli_parse_stack<R>(f: impl FnOnce() -> R) -> R {
    f()
}

#[test]
fn parse_completions_bash() {
    with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "completions", "bash"]).expect("completions bash");
    });
}

#[test]
fn parse_global_color_and_build() {
    let r = with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "--color", "never", "build", "foo.vox"])
            .expect("build with global color")
    });
    assert!(r.global.color.is_some());
}

#[test]
fn parse_fabrica_build() {
    with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "fabrica", "build", "foo.vox"]).expect("fabrica build");
    });
}

#[test]
fn parse_fabrica_visible_alias_fab() {
    with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "fab", "build", "foo.vox"]).expect("fab build");
    });
}

#[test]
fn parse_secrets_routes_to_secrets_doctor() {
    let r = with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "secrets", "doctor"]).expect("secrets doctor")
    });
    assert!(matches!(
        r.cmd,
        Cli::Secrets {
            cmd: SecretsCmd::Status { .. }
        }
    ));
}

#[test]
fn parse_clavis_alias_routes_to_secrets_doctor() {
    let r = with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "clavis", "doctor"]).expect("clavis doctor")
    });
    assert!(matches!(
        r.cmd,
        Cli::Secrets {
            cmd: SecretsCmd::Status { .. }
        }
    ));
}

#[test]
fn root_long_help_mentions_recommended_catalog() {
    let help = with_cli_parse_stack(|| {
        let mut cmd = VoxCliRoot::command();
        cmd.render_long_help().to_string()
    });
    assert!(
        help.contains("commands") && help.contains("recommended"),
        "help should steer users to `vox commands --recommended`"
    );
}

#[test]
fn mens_long_help_canonical_train_is_mens_not_schola() {
    let help = with_cli_parse_stack(|| {
        let mut root = VoxCliRoot::command();
        let mens = root
            .find_subcommand_mut("mens")
            .expect("vox mens subcommand should exist with default features");
        mens.render_long_help().to_string()
    });
    assert!(
        help.contains("vox mens train"),
        "mens quick-start should show registry-canonical `vox mens train`"
    );
    assert!(
        !help.contains("vox schola train"),
        "avoid stale `vox schola train` in user-facing mens help"
    );
}

#[test]
fn parse_db_explain_subcommand() {
    with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from([
            "vox",
            "db",
            "explain",
            "--file",
            "examples/golden/crud_api.vox",
        ])
        .expect("db explain parse");
    });
}

#[test]
fn parse_db_explain_jsonl_flag() {
    with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from([
            "vox",
            "db",
            "explain",
            "--file",
            "examples/golden/db_native_ir.vox",
            "--jsonl",
        ])
        .expect("db explain --jsonl parse");
    });
}

#[test]
fn parse_package_management_verbs() {
    with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "add", "foo", "--version", "^1"]).expect("add");
        VoxCliRoot::try_parse_from(["vox", "remove", "foo"]).expect("remove");
        VoxCliRoot::try_parse_from(["vox", "update"]).expect("update");
        VoxCliRoot::try_parse_from(["vox", "lock"]).expect("lock");
        VoxCliRoot::try_parse_from(["vox", "lock", "--locked"]).expect("lock --locked");
        VoxCliRoot::try_parse_from(["vox", "sync"]).expect("sync");
        VoxCliRoot::try_parse_from(["vox", "sync", "--frozen"]).expect("sync --frozen");
        VoxCliRoot::try_parse_from(["vox", "upgrade", "--channel", "stable"]).expect("upgrade");
        VoxCliRoot::try_parse_from([
            "vox",
            "upgrade",
            "--apply",
            "--provider",
            "github",
            "--allow-breaking",
        ])
        .expect("upgrade --apply");
        VoxCliRoot::try_parse_from([
            "vox",
            "upgrade",
            "--provider",
            "http",
            "--base-url",
            "https://example.com/org/vox/releases",
            "--version",
            "v1.0.0",
        ])
        .expect("upgrade http mirror");
        VoxCliRoot::try_parse_from([
            "vox",
            "upgrade",
            "--source",
            "repo",
            "--repo-root",
            ".",
            "--ref",
            "v1.0.0",
        ])
        .expect("upgrade --source repo");
    });
}

#[test]
fn parse_pm_subcommands() {
    with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "pm", "search", "httpx"]).expect("pm search");
        VoxCliRoot::try_parse_from(["vox", "pm", "info", "pkg"]).expect("pm info");
        VoxCliRoot::try_parse_from([
            "vox",
            "pm",
            "publish",
            "n",
            "--version",
            "0.1.0",
            "--file",
            "Cargo.toml",
        ])
        .expect("pm publish");
        VoxCliRoot::try_parse_from([
            "vox",
            "pm",
            "mirror",
            "pkg-a",
            "--version",
            "1.2.3",
            "--file",
            "blob.bin",
        ])
        .expect("pm mirror --file");
        VoxCliRoot::try_parse_from([
            "vox",
            "pm",
            "mirror",
            "pkg-b",
            "--version",
            "0.0.9",
            "--from-registry",
            "http://127.0.0.1:9",
        ])
        .expect("pm mirror --from-registry");
        VoxCliRoot::try_parse_from(["vox", "pm", "cache", "status"]).expect("pm cache status");
        VoxCliRoot::try_parse_from(["vox", "pm", "cache", "clear"]).expect("pm cache clear");
        VoxCliRoot::try_parse_from(["vox", "pm", "yank", "oldpkg", "--version", "0.1.0"])
            .expect("pm yank");
        VoxCliRoot::try_parse_from(["vox", "pm", "vendor"]).expect("pm vendor");
        VoxCliRoot::try_parse_from(["vox", "pm", "vendor", "--dir", "third_party/vox_vendor"])
            .expect("pm vendor --dir");
        VoxCliRoot::try_parse_from(["vox", "pm", "verify"]).expect("pm verify");
        VoxCliRoot::try_parse_from(["vox", "pm", "verify", "--registry", "http://127.0.0.1:9"])
            .expect("pm verify --registry");
    });
}

#[test]
fn install_subcommand_removed_phase_b() {
    let err = with_cli_parse_stack(|| {
        match VoxCliRoot::try_parse_from(["vox", "install", "legacy-pkg"]) {
            Ok(_) => panic!("expected parse failure for removed subcommand `install`"),
            Err(e) => e,
        }
    });
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("install") && (msg.contains("unrecognized") || msg.contains("unexpected")),
        "clap should reject removed subcommand `install`: {msg}"
    );
}

#[test]
fn parse_ci_pm_provenance() {
    with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "ci", "pm-provenance"]).expect("pm-provenance");
        VoxCliRoot::try_parse_from([
            "vox",
            "ci",
            "pm-provenance",
            "--strict",
            "--root",
            "packages/foo",
        ])
        .expect("pm-provenance strict + root");
    });
}

#[test]
fn parse_ci_rust_ecosystem_policy() {
    with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "ci", "rust-ecosystem-policy"])
            .expect("rust-ecosystem-policy");
    });
}

#[test]
fn parse_ci_policy_smoke() {
    let r = with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "ci", "policy-smoke"]).expect("policy-smoke")
    });
    assert!(matches!(
        r.cmd,
        Cli::Ci {
            cmd: CiCmd::PolicySmoke
        }
    ));
}

#[test]
fn parse_ci_compile_matrix() {
    with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "ci", "compile-matrix"]).expect("compile-matrix");
    });
}

#[test]
fn parse_ci_retirement_audit() {
    let r = with_cli_parse_stack(|| {
        VoxCliRoot::try_parse_from(["vox", "ci", "retirement-audit"]).expect("retirement-audit")
    });
    assert!(matches!(
        r.cmd,
        Cli::Ci {
            cmd: CiCmd::RetirementAudit
        }
    ));
}
