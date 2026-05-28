//! CR-L7 — `vox new web` → `vox deploy --dry-run` → `vox doctor --project`
//! end-to-end gate.
//!
//! Proves the production-shaped flow for the audit-recommended default:
//! scaffolding the **web** template yields a project that
//!
//!   1. carries a portable `[deploy] target = "container"` block (with the
//!      Fly stanza commented out — see `vox-project-scaffold/src/lib.rs`),
//!   2. dry-runs `vox deploy` without requiring docker/podman installed
//!      (the `--dry-run` short-circuit in `commands::deploy::run`), and
//!   3. compile-checks GREEN under `vox doctor --project`.
//!
//! This is the third leg of the CR-L7 audit gate ratified in the v1.0
//! completion plan and the P4 platform architecture audit
//! (`docs/superpowers/specs/2026-05-18-p4-platform-architecture-audit.md`).
//!
//! Marked `#[serial]` because `commands::deploy::run` reads `Vox.toml` from
//! the process-global current working directory.

use std::path::Path;

use serial_test::serial;
use vox_cli::cli_args::DeployArgs;
use vox_cli::commands::deploy as deploy_cmd;
use vox_cli::commands::diagnostics::doctor::project_check;
use vox_project_scaffold::scaffold_vox_project_at;

fn read_manifest(root: &Path) -> String {
    std::fs::read_to_string(root.join("Vox.toml")).expect("read Vox.toml")
}

#[tokio::test]
#[serial]
async fn cr_l7_web_scaffold_deploys_container_dry_run_and_doctor_green() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // 1. Scaffold the audit-recommended `web` template.
    let summary = scaffold_vox_project_at(tmp.path(), "cr-l7-web", "application", Some("web"))
        .expect("scaffold web template");
    assert_eq!(summary.template_applied.as_deref(), Some("web"));

    // 2. Manifest carries the portable container default + commented Fly stanza.
    let manifest = read_manifest(tmp.path());
    assert!(
        manifest.contains("[deploy]"),
        "scaffold should emit a [deploy] section for web template; got:\n{manifest}"
    );
    assert!(
        manifest.contains("target = \"container\""),
        "default deploy.target should be \"container\"; got:\n{manifest}"
    );
    assert!(
        manifest.contains("# [deploy.fly]"),
        "fly stanza should be present but commented; got:\n{manifest}"
    );

    // 3. `vox doctor --project` is GREEN over the scaffolded sources.
    project_check::run(tmp.path(), true)
        .await
        .expect("doctor --project should be GREEN on a fresh web scaffold");

    // 4. `vox deploy --dry-run` runs the planner without requiring docker.
    //    `deploy::run` reads Vox.toml from the cwd, so chdir under #[serial].
    let original_cwd = std::env::current_dir().expect("get cwd");
    std::env::set_current_dir(tmp.path()).expect("chdir into scaffold");

    let deploy_result = deploy_cmd::run(DeployArgs {
        environment: "production".to_string(),
        target: None,
        runtime: None,
        dry_run: true,
        detach: false,
        locked: false,
    })
    .await;

    // Always restore cwd before asserting so a failure doesn't poison
    // subsequent tests in the same process.
    std::env::set_current_dir(&original_cwd).expect("restore cwd");

    deploy_result.expect("dry-run deploy should succeed without a container runtime");
}

/// Every shipped template should scaffold a project that compiles GREEN —
/// otherwise users hit a broken starter on first `vox new`. This catches
/// drift between the compiler's accepted surface and the template source.
#[tokio::test]
async fn every_template_scaffolds_doctor_green() {
    for (kind, template) in [
        ("application", None),
        ("application", Some("web")),
        ("application", Some("api")),
        ("application", Some("chatbot")),
    ] {
        let tmp = tempfile::tempdir().expect("tempdir");
        scaffold_vox_project_at(tmp.path(), "tpl-probe", kind, template)
            .expect("scaffold should succeed");
        let result = project_check::run(tmp.path(), true).await;
        assert!(
            result.is_ok(),
            "template kind={kind:?} template={template:?} should be doctor-GREEN; got: {result:?}"
        );
    }
}

#[tokio::test]
#[serial]
async fn cr_l7_default_application_has_no_deploy_block() {
    // Counter-example: the plain `application` kind (no `web` template)
    // intentionally omits `[deploy]` — only the web template opts the
    // user into the portable deploy lane today.
    let tmp = tempfile::tempdir().expect("tempdir");
    scaffold_vox_project_at(tmp.path(), "no-deploy", "application", None).expect("scaffold");
    let manifest = read_manifest(tmp.path());
    assert!(
        !manifest.contains("[deploy]"),
        "non-web scaffolds should not preconfigure deploy; got:\n{manifest}"
    );
}
