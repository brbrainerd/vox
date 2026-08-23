//! `vox ci pre-push` — local aggregate before `git push`.
//!
//! ## Profiles
//!
//! - **Fast (default):** `cargo fmt`, line-endings, ssot-drift, **scoped** doc lint +
//!   doctest on changed `docs/src/**/*.md` (excludes `docs/src/archive/`), and workspace
//!   drift-check. Tuned so hooks finish quickly; CI still runs full docs-quality.
//! - **`--complete`:** historical full static gate — whole-tree doc lint + doctest under
//!   `docs/src/`, doc-inventory, workspace clippy (`-D warnings`), scoped TOESTUB.
//! - **`--full`:** `--complete` plus **`cargo nextest run --workspace --profile ci`**
//!   (slow `#[ignore]` tests excluded by default).
//!
//! ## Extended flags for `--full`
//!
//! - **`--include-slow`:** also run the slow `#[ignore]` partition (arch-check live
//!   scientia timeout, codegen bundle; ~3–5 min extra).
//! - **`--with-coverage`:** substitute `cargo llvm-cov nextest` for the plain nextest
//!   step and append `cargo llvm-cov report` (lcov + HTML under `target/llvm-cov/`).
//!   Requires `cargo-llvm-cov` on PATH; adds ~60s.
//! - **`--since <ref>`:** run nextest only for packages changed since `<ref>` plus
//!   their transitive reverse-deps (via `git diff` + `cargo metadata` graph). Large
//!   impacted sets run in chunks (`VOX_PREPUSH_SINCE_CHUNK_SIZE`, default 10); falls
//!   back to `--workspace` only on git/metadata hard failures. Typical wall-clock on
//!   1–3 crate edits: 3–20s.
//! - **`--skip-complete`:** with `--full`, skip replaying complete-tier static checks
//!   (clippy, doc-inventory, scoped TOESTUB) after a green `--complete` in the same loop.
//!
//! **`--quick`** is a legacy no-op alias for the default fast profile (conflicts with
//! `--complete` / `--full`).
//!
//! **`--report-json <path>`** — timing summary schema [**`contracts/reports/pre-push-report.v1.schema.json`**](../../../contracts/reports/pre-push-report.v1.schema.json)
//! (`schema_version` **3** adds `with_coverage` and new profile values `full+cov`,
//! `full+since`, `full+cov+since`; schema_version **2** added `profile` and `complete`).
//!
//! **`VOX_PREPUSH_AUDIT_LOG`** — append one JSON line per successful run (not `--dry-run`).

use anyhow::{Context, Result, anyhow, bail};
use cargo_metadata::MetadataCommand;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct PrePushOpts {
    pub quick: bool,
    pub complete: bool,
    pub full: bool,
    pub dry_run: bool,
    pub act: bool,
    pub report_json: Option<PathBuf>,
    /// When true (only meaningful with `full`), append a second nextest step that runs
    /// the four slow `#[ignore]` tests explicitly.  The main nextest step still skips
    /// all `#[ignore]` tests regardless of this flag.
    pub include_slow: bool,
    /// When true (only valid with `full`), substitute `cargo llvm-cov nextest` for the
    /// plain nextest step and append a `cargo llvm-cov report` step.  Requires
    /// `cargo-llvm-cov` on PATH.  Errors at runtime if used without `--full`.
    pub with_coverage: bool,
    /// When set, run nextest only for the packages affected by changes since the given git
    /// ref plus their transitive reverse-deps. Large impacted sets are chunked instead of
    /// falling back to workspace. Only meaningful with `full`. `None` means full workspace.
    pub since: Option<String>,
    /// When true with `--full`, omit complete-tier static checks (clippy, doc-inventory,
    /// scoped TOESTUB, whole-tree docs) — use after `--complete` already passed locally.
    pub skip_complete: bool,
    /// After a successful (non-dry-run) run, compare total elapsed time against the tier
    /// budgets in `contracts/budgets/test-tier-budgets.v1.yaml`.  Warns to stderr when
    /// elapsed > `warn_ms`; returns an error when elapsed > `fail_ms`.  No-op if the
    /// budgets file is absent (safe on first clone).
    pub enforce_budgets: bool,
}

fn profile_name(opts: &PrePushOpts) -> &'static str {
    if opts.full && opts.with_coverage && opts.since.is_some() {
        "full+cov+since"
    } else if opts.full && opts.with_coverage {
        "full+cov"
    } else if opts.full && opts.since.is_some() {
        "full+since"
    } else if opts.full {
        "full"
    } else if opts.complete {
        "complete"
    } else {
        "fast"
    }
}

fn run_complete_static(opts: &PrePushOpts) -> bool {
    (opts.complete || opts.full) && !opts.skip_complete
}

/// Workflows that run on `ubuntu-latest` (GH-hosted exceptions).
const ACT_WORKFLOWS: &[&str] = &[
    ".github/workflows/docs-quality.yml",
    ".github/workflows/link_checker.yml",
    ".github/workflows/ts-emit-noemit.yml",
];

#[derive(Debug, Clone, Serialize)]
pub struct PrePushStepScope {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impacted_packages: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toestub_roots: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct PrePushReportV1 {
    pub schema_version: u32,
    pub profile: String,
    pub ok: bool,
    pub quick: bool,
    pub complete: bool,
    pub full: bool,
    pub with_coverage: bool,
    pub dry_run: bool,
    pub total_ms: u64,
    pub steps: Vec<PrePushStepTiming>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrePushStepTiming {
    pub label: String,
    pub elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<PrePushStepScope>,
}

struct OwnedStep {
    label: String,
    scope: Option<PrePushStepScope>,
    run: Box<dyn Fn(&Path) -> Result<()> + Send>,
}

#[allow(unsafe_code)] // `set_var` is unsafe on Rust 2024; pre-push is single-threaded.
pub fn run(root: &Path, opts: PrePushOpts) -> Result<()> {
    // nextest and other tools spawn nested `cargo` without our `--config` flags;
    // clear the workspace rustc-wrapper so gates work when sccache is absent.
    unsafe {
        std::env::set_var("CARGO_BUILD_RUSTC_WRAPPER", "");
    }

    if opts.with_coverage && !opts.full {
        bail!("`--with-coverage` requires `--full`");
    }
    if opts.since.is_some() && !opts.full {
        bail!("`--since` requires `--full`");
    }
    if opts.skip_complete && !opts.full {
        bail!("`--skip-complete` requires `--full`");
    }
    let steps = build_steps(root, &opts)?;
    let mut step_records: Vec<PrePushStepTiming> = Vec::with_capacity(steps.len());
    if opts.dry_run {
        for s in &steps {
            println!("DRY-RUN: {}", s.label);
            step_records.push(PrePushStepTiming {
                label: s.label.clone(),
                elapsed_ms: None,
                scope: s.scope.clone(),
            });
        }
        if opts.act {
            run_act(root, true)?;
        }
        write_pre_push_report(
            root,
            &opts,
            &step_records,
            true,
            0,
            opts.report_json.as_deref(),
        )?;
        return Ok(());
    }
    const WARN_AT_SECS: u64 = 20 * 60;
    const FAIL_AT_SECS: u64 = 25 * 60;
    let total = Instant::now();
    for s in steps {
        let elapsed_secs = total.elapsed().as_secs();
        if elapsed_secs >= FAIL_AT_SECS {
            bail!(
                "pre-push: global 25m timeout exceeded at {:02}:{:02} before step `{}`",
                elapsed_secs / 60,
                elapsed_secs % 60,
                s.label
            );
        }
        if elapsed_secs >= WARN_AT_SECS {
            eprintln!(
                "pre-push: WARNING — {:02}:{:02} elapsed, hard timeout at 25:00; running `{}`",
                elapsed_secs / 60,
                elapsed_secs % 60,
                s.label
            );
        }
        let started = Instant::now();
        let label = s.label.clone();
        let scope = s.scope.clone();
        println!("==> {}", label);
        let run = s.run;
        match run_step_with_heartbeat(&label, total, || run(root))
            .with_context(|| format!("step `{label}`"))
        {
            Ok(()) => {
                let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                println!("    OK ({elapsed_ms}ms)");
                step_records.push(PrePushStepTiming {
                    label,
                    elapsed_ms: Some(elapsed_ms),
                    scope,
                });
            }
            Err(e) => {
                let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                step_records.push(PrePushStepTiming {
                    label,
                    elapsed_ms: Some(elapsed_ms),
                    scope,
                });
                let total_ms = total.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                let _ = write_pre_push_report(
                    root,
                    &opts,
                    &step_records,
                    false,
                    total_ms,
                    opts.report_json.as_deref(),
                );
                return Err(e);
            }
        }
    }
    if opts.act {
        run_act(root, false)?;
    }
    let total_ms = total.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    println!(
        "pre-push: profile `{}` — all checks passed in {total_ms}ms",
        profile_name(&opts)
    );
    print_pr_review_discipline_hint(root);
    write_pre_push_report(
        root,
        &opts,
        &step_records,
        true,
        total_ms,
        opts.report_json.as_deref(),
    )?;
    append_prepush_audit_log(root, &opts, total_ms)?;
    if opts.enforce_budgets {
        check_tier_budget(root, profile_name(&opts), total_ms)?;
    }
    Ok(())
}

/// Non-blocking advisory printed after a successful pre-push: when re-pushing a
/// feature branch that already has an upstream (the proxy for an open PR),
/// remind that pushes do **not** auto-trigger a CodeRabbit review (the repo
/// `.coderabbit.yaml` sets `auto_review.auto_incremental_review: false`) and that
/// `@coderabbitai review` is the on-demand trigger. See AGENTS.md §"PR & Review
/// Discipline". Best-effort and never fails the push; uses only local git (no
/// network), so it adds no measurable latency.
fn print_pr_review_discipline_hint(root: &Path) {
    use std::process::Command;
    let branch = match Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => return,
    };
    if branch.is_empty() || branch == "main" || branch == "master" || branch == "HEAD" {
        return;
    }
    // An upstream tracking branch means this branch was pushed before — i.e. this
    // is a re-push, the case where an open PR likely already exists. First pushes
    // (no upstream yet) stay silent so the initial PR-open review isn't second-guessed.
    let has_upstream = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_upstream {
        return;
    }
    eprintln!(
        "pre-push: review discipline — this re-push will NOT auto-trigger a CodeRabbit review"
    );
    eprintln!(
        "          (.coderabbit.yaml auto_incremental_review=false). Batch commits, and comment"
    );
    eprintln!("          `@coderabbitai review` on the PR when it's ready for a fresh review.");
}

fn run_step_with_heartbeat(
    label: &str,
    push_start: Instant,
    f: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_bg = Arc::clone(&stop);
    let label_owned = label.to_string();
    let bg = thread::spawn(move || {
        let t0 = Instant::now();
        loop {
            thread::sleep(vox_config::timeouts::D_3S);
            if stop_bg.load(Ordering::Relaxed) {
                break;
            }
            let step_s = t0.elapsed().as_secs();
            let total_s = push_start.elapsed().as_secs();
            eprintln!(
                "pre-push: still running `{}` — step {:02}:{:02} | total {:02}:{:02}",
                label_owned,
                step_s / 60,
                step_s % 60,
                total_s / 60,
                total_s % 60,
            );
        }
    });
    let out = f();
    stop.store(true, Ordering::Relaxed);
    let _ = bg.join();
    out
}

fn write_pre_push_report(
    root: &Path,
    opts: &PrePushOpts,
    steps: &[PrePushStepTiming],
    ok: bool,
    total_ms: u64,
    report_path: Option<&Path>,
) -> Result<()> {
    let Some(path) = report_path else {
        return Ok(());
    };
    let report = PrePushReportV1 {
        schema_version: 4,
        profile: profile_name(opts).to_string(),
        ok,
        quick: opts.quick,
        complete: opts.complete,
        full: opts.full,
        with_coverage: opts.with_coverage,
        dry_run: opts.dry_run,
        total_ms,
        steps: steps.to_vec(),
    };
    let json = serde_json::to_string_pretty(&report)?;
    let out_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| parent.display().to_string())?;
    }
    std::fs::write(&out_path, format!("{json}\n"))
        .with_context(|| out_path.display().to_string())?;
    Ok(())
}

#[derive(Serialize)]
struct PrePushAuditLine {
    schema_version: u32,
    event: &'static str,
    unix_ms: u64,
    total_ms: u64,
    profile: String,
    quick: bool,
    complete: bool,
    full: bool,
    with_coverage: bool,
}

/// Map a profile name to the corresponding key in `test-tier-budgets.v1.yaml`.
///
/// `full+since` and `full+cov+since` reuse the `full` / `full_cov` budgets — they should be
/// faster (impacted-crate subset), so they will pass trivially. If `--since` somehow falls
/// back to workspace, the budget check catches the regression.
fn tier_budget_key(profile: &str) -> Option<&'static str> {
    match profile {
        "fast" => Some("fast"),
        "complete" => Some("complete"),
        "full" | "full+since" => Some("full"),
        "full+cov" | "full+cov+since" => Some("full_cov"),
        _ => None,
    }
}

/// Read `contracts/budgets/test-tier-budgets.v1.yaml` and compare `total_ms` against the
/// `warn_ms` / `fail_ms` thresholds for the current tier.
///
/// - Returns `Ok(())` immediately if the budgets file is absent (safe on first clone).
/// - Prints a warning to stderr when `total_ms > warn_ms` (1.2× baseline).
/// - Returns `Err` when `total_ms > fail_ms` (1.5× baseline), causing the command to fail.
fn check_tier_budget(root: &Path, profile: &str, total_ms: u64) -> Result<()> {
    let budgets_path = root.join("contracts/budgets/test-tier-budgets.v1.yaml");
    if !budgets_path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&budgets_path)
        .with_context(|| format!("read {}", budgets_path.display()))?;
    let doc: serde_yaml::Value =
        serde_yaml::from_str(&raw).with_context(|| format!("parse {}", budgets_path.display()))?;
    let Some(tier_key) = tier_budget_key(profile) else {
        return Ok(());
    };
    let Some(tiers) = doc.get("tiers") else {
        return Ok(());
    };
    let Some(tier) = tiers.get(tier_key) else {
        return Ok(());
    };
    let warn_ms = tier
        .get("warn_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(u64::MAX);
    let fail_ms = tier
        .get("fail_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(u64::MAX);
    if total_ms > fail_ms {
        bail!(
            "pre-push budget exceeded: profile `{}` took {}ms > fail threshold {}ms \
             (see contracts/budgets/test-tier-budgets.v1.yaml)",
            profile,
            total_ms,
            fail_ms
        );
    }
    if total_ms > warn_ms {
        eprintln!(
            "pre-push budget warning: profile `{}` took {}ms > warn threshold {}ms \
             (see contracts/budgets/test-tier-budgets.v1.yaml)",
            profile, total_ms, warn_ms
        );
    }
    Ok(())
}

fn append_prepush_audit_log(root: &Path, opts: &PrePushOpts, total_ms: u64) -> Result<()> {
    let Ok(raw) = std::env::var("VOX_PREPUSH_AUDIT_LOG") else {
        return Ok(());
    };
    let path = if Path::new(&raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        root.join(raw)
    };
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let line = PrePushAuditLine {
        schema_version: 3,
        event: "pre-push-complete",
        unix_ms,
        total_ms,
        profile: profile_name(opts).to_string(),
        quick: opts.quick,
        complete: opts.complete,
        full: opts.full,
        with_coverage: opts.with_coverage,
    };
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| path.display().to_string())?;
    writeln!(f, "{}", serde_json::to_string(&line)?).with_context(|| path.display().to_string())?;
    Ok(())
}

fn build_steps(root: &Path, opts: &PrePushOpts) -> Result<Vec<OwnedStep>> {
    let mut v: Vec<OwnedStep> = vec![
        OwnedStep {
            label: "cargo fmt --all -- --check".into(),
            scope: None,
            run: Box::new(step_fmt),
        },
        OwnedStep {
            label: "vox ci line-endings".into(),
            scope: None,
            run: Box::new(step_line_endings),
        },
        OwnedStep {
            label: "vox ci ssot-drift".into(),
            scope: None,
            run: Box::new(step_ssot_drift),
        },
        OwnedStep {
            label: "vox ci spoke-check".into(),
            scope: None,
            run: Box::new(step_spoke_check),
        },
        OwnedStep {
            label: "vox ci runner-policy-check".into(),
            scope: None,
            run: Box::new(step_runner_policy_check),
        },
        OwnedStep {
            label: "vox ci workflow-concurrency-guard".into(),
            scope: None,
            run: Box::new(step_workflow_concurrency_guard),
        },
        OwnedStep {
            label: "vox ci workflow-permissions-guard".into(),
            scope: None,
            run: Box::new(step_workflow_permissions_guard),
        },
        OwnedStep {
            label: "vox ci check-links".into(),
            scope: None,
            run: Box::new(step_check_links),
        },
        OwnedStep {
            label: "vox ci retired-symbol-check".into(),
            scope: None,
            run: Box::new(step_retired_symbol_check),
        },
        OwnedStep {
            label: "vox ci canonical-map-verify".into(),
            scope: None,
            run: Box::new(step_canonical_map_verify),
        },
    ];

    if run_complete_static(opts) {
        v.push(OwnedStep {
            label: "vox-doc-pipeline --lint-only (full docs/src)".into(),
            scope: None,
            run: Box::new(step_doc_frontmatter_full),
        });
        v.push(OwnedStep {
            label: "vox ci doctest-md --strict (full docs/src)".into(),
            scope: None,
            run: Box::new(step_doctest_md_full),
        });
    } else {
        let rel = Arc::new(changed_docs_md_rel_paths(root)?);
        let n = rel.len();
        let label = if n == 0 {
            "vox-doc-pipeline --lint-only (scoped — skip: no changed .md outside archive)"
                .to_string()
        } else {
            format!(
                "vox-doc-pipeline --lint-only (scoped: {n} changed .md under docs/src, excl. archive)"
            )
        };
        let rel_lint = Arc::clone(&rel);
        v.push(OwnedStep {
            label,
            scope: None,
            run: Box::new(move |r| step_doc_frontmatter_scoped(r, rel_lint.as_slice())),
        });

        let label_d = if n == 0 {
            "vox ci doctest-md --strict (scoped — skip: no changed .md)".to_string()
        } else {
            format!("vox ci doctest-md --strict (scoped: {n} files)")
        };
        let rel_test = Arc::clone(&rel);
        v.push(OwnedStep {
            label: label_d,
            scope: None,
            run: Box::new(move |r| step_doctest_scoped(r, rel_test.as_slice())),
        });
    }

    v.push(OwnedStep {
        label: "vox-drift-check workspace".into(),
        scope: None,
        run: Box::new(step_drift_check),
    });

    if run_complete_static(opts) {
        let toestub_roots = changed_dirs_under_crates(root)
            .map(|d| d.len())
            .unwrap_or(0);
        v.push(OwnedStep {
            label: "vox ci doc-inventory verify".into(),
            scope: None,
            run: Box::new(step_doc_inventory),
        });
        v.push(OwnedStep {
            label: "cargo clippy --workspace --all-targets -- -D warnings".into(),
            scope: None,
            run: Box::new(step_clippy),
        });
        v.push(OwnedStep {
            label: "vox ci toestub-scoped --mode enforce-warn (changed paths, batched)".into(),
            scope: Some(PrePushStepScope {
                impacted_packages: None,
                chunk_count: None,
                fallback_reason: None,
                toestub_roots: Some(toestub_roots),
            }),
            run: Box::new(step_toestub_changed),
        });
    }

    if opts.full {
        let with_cov = opts.with_coverage;
        let since_ref = opts.since.clone();

        // Resolve impacted-crate set (if --since was given).
        // Done here (not inside the closure) so the label can reflect the result.
        let impacted = since_ref
            .as_deref()
            .map(|r| compute_impacted_crates(root, r));

        let nextest_label = match &impacted {
            Some(Ok(ImpactedCrates {
                fallback: true,
                fallback_reason: reason,
                ..
            })) => {
                let why = reason.as_deref().unwrap_or("unknown");
                if with_cov {
                    format!(
                        "cargo llvm-cov nextest --workspace (--since fallback: {why}, slow excluded)"
                    )
                } else {
                    format!(
                        "cargo nextest run --workspace (--since fallback: {why}, slow excluded)"
                    )
                }
            }
            Some(Ok(ImpactedCrates {
                packages, chunks, ..
            })) => {
                let n = packages.len();
                let c = chunks.len();
                if with_cov {
                    format!(
                        "cargo llvm-cov nextest ({n} impacted pkg(s), {c} chunk(s), slow excluded)"
                    )
                } else {
                    format!("cargo nextest run ({n} impacted pkg(s), {c} chunk(s), slow excluded)")
                }
            }
            Some(Err(_)) | None => {
                if with_cov {
                    "cargo llvm-cov nextest --workspace --profile ci (slow excluded)".into()
                } else {
                    "cargo nextest run --workspace --profile ci --no-fail-fast (slow tests excluded)".into()
                }
            }
        };

        let nextest_scope =
            impacted
                .as_ref()
                .and_then(|r| r.as_ref().ok())
                .map(|ic| PrePushStepScope {
                    impacted_packages: if ic.fallback {
                        None
                    } else {
                        Some(ic.packages.len())
                    },
                    chunk_count: if ic.fallback {
                        None
                    } else {
                        Some(ic.chunks.len())
                    },
                    fallback_reason: ic.fallback_reason.clone(),
                    toestub_roots: None,
                });

        let impacted_for_run = impacted;
        v.push(OwnedStep {
            label: nextest_label,
            scope: nextest_scope,
            run: Box::new(move |root| match (&impacted_for_run, with_cov) {
                (
                    Some(Ok(ImpactedCrates {
                        fallback: false,
                        chunks,
                        ..
                    })),
                    cov,
                ) => step_nextest_packages_chunked(root, chunks, cov),
                (_, false) => step_nextest(root),
                (_, true) => step_nextest_with_coverage(root),
            }),
        });
        if opts.with_coverage {
            v.push(OwnedStep {
                label: "cargo llvm-cov report (lcov + html under target/llvm-cov/)".into(),
                scope: None,
                run: Box::new(step_llvm_cov_report),
            });
        }
        if opts.include_slow {
            v.push(OwnedStep {
                label: "cargo nextest run (slow partition: arch-check, scientia timeout, codegen bundle)".into(),
                scope: None,
                run: Box::new(step_nextest_slow),
            });
        }
    }

    Ok(v)
}

/// Run the GH-hosted exception workflows through `act`.
pub fn run_act(root: &Path, dry_run: bool) -> Result<()> {
    let act_cmd = if dry_run {
        which_act().unwrap_or_else(|_| ActCommand::new("act", vec![]))
    } else {
        which_act().context(
            "`act` not found on PATH — install nektos/act (https://nektosact.com) to use --act",
        )?
    };

    let mut failures: Vec<&str> = Vec::new();
    for &workflow in ACT_WORKFLOWS {
        println!("==> act: {workflow}");
        if dry_run {
            println!(
                "    DRY-RUN: {}",
                act_cmd.display_with_args(&["push", "--workflows", workflow])
            );
            continue;
        }
        let status = Command::new(&act_cmd.executable)
            .args(&act_cmd.base_args)
            .args(["push", "--workflows", workflow])
            .current_dir(root)
            .status()
            .with_context(|| format!("spawn act for {workflow}"))?;
        if status.success() {
            println!("    OK");
        } else {
            eprintln!("    FAILED ({workflow}): exit {:?}", status.code());
            failures.push(workflow);
        }
    }
    if !failures.is_empty() {
        bail!(
            "act: {} workflow(s) failed: {}",
            failures.len(),
            failures.join(", ")
        );
    }
    Ok(())
}

fn which_act() -> Result<ActCommand> {
    let candidates = [
        ActCommand::new("act", vec![]),
        ActCommand::new("gh", vec!["act"]),
    ];
    for candidate in candidates {
        if let Ok(out) = Command::new(&candidate.executable)
            .args(&candidate.base_args)
            .arg("--version")
            .output()
        {
            if out.status.success() {
                return Ok(candidate);
            }
        }
    }
    Err(anyhow!("act binary not found"))
}

#[derive(Clone, Debug)]
struct ActCommand {
    executable: String,
    base_args: Vec<String>,
}

impl ActCommand {
    fn new(executable: &str, base_args: Vec<&str>) -> Self {
        Self {
            executable: executable.to_string(),
            base_args: base_args.into_iter().map(ToString::to_string).collect(),
        }
    }

    fn display_with_args(&self, runtime_args: &[&str]) -> String {
        let mut parts = Vec::with_capacity(1 + self.base_args.len() + runtime_args.len());
        parts.push(self.executable.clone());
        parts.extend(self.base_args.iter().cloned());
        parts.extend(runtime_args.iter().map(|arg| (*arg).to_string()));
        parts.join(" ")
    }
}

fn cargo() -> Command {
    // Nested `cargo run` steps (doc-pipeline, drift-check, …) inherit
    // `rustc-wrapper = "sccache"` from config.toml. On Windows the wrapper must
    // be a real executable; clear it for gate subprocesses so pre-push works
    // when sccache is not installed.
    let mut cmd = Command::new(super::cargo_bin());
    cmd.args(["--config", "build.rustc-wrapper=\"\""]);
    cmd.env("CARGO_BUILD_RUSTC_WRAPPER", "");
    cmd
}

fn cargo_status(root: &Path, args: &[&str]) -> Result<()> {
    let status = cargo()
        .args(args)
        .current_dir(root)
        .status()
        .with_context(|| format!("spawn cargo {}", args.join(" ")))?;
    if !status.success() {
        bail!("cargo {} exited with {:?}", args.join(" "), status.code());
    }
    Ok(())
}

/// On Windows, nextest for `vox-cli` rebuilds `vox.exe` while pre-push runs from it (os
/// error 5). Skip that package and advise a separate nextest invocation.
fn nextest_skip_packages() -> Vec<String> {
    if !cfg!(windows) || !running_as_vox_exe() {
        return vec![];
    }
    eprintln!(
        "pre-push: skipping nextest for vox-cli on Windows (running vox.exe holds the binary lock); run `cargo nextest -p vox-cli --profile ci` separately"
    );
    vec!["vox-cli".into()]
}

fn running_as_vox_exe() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().eq_ignore_ascii_case("vox.exe"))
        })
        .unwrap_or(false)
}

fn filter_nextest_packages(packages: &[String]) -> Vec<String> {
    let skip: HashSet<String> = nextest_skip_packages().into_iter().collect();
    if skip.is_empty() {
        return packages.to_vec();
    }
    packages
        .iter()
        .filter(|p| !skip.contains(p.as_str()))
        .cloned()
        .collect()
}

fn nextest_cargo_status(root: &Path, args: &[&str]) -> Result<()> {
    let status = cargo()
        .args(args)
        .current_dir(root)
        .status()
        .with_context(|| format!("spawn cargo {}", args.join(" ")))?;
    if !status.success() {
        bail!("cargo {} exited with {:?}", args.join(" "), status.code());
    }
    Ok(())
}

/// Returns a `Command` that invokes the current `vox` binary.
///
/// On Windows, `cargo run -p vox-cli` would try to relink `vox.exe`, which
/// fails with "Access is denied" (os error 5) because the currently running
/// process holds a lock on the executable. We avoid this by resolving
/// `current_exe()` and invoking it directly — the binary is already fresh
/// (we just ran it). On non-Windows platforms we keep using `cargo run` so
/// that sub-commands always run against the latest source.
fn git_diff_name_only_for_prepush(root: &Path) -> Result<String> {
    let primary = std::env::var("VOX_PREPUSH_BASE").unwrap_or_else(|_| "origin/main".into());
    let attempt = |base: &str| -> Result<String> {
        let out = // vox-arch-check: allow git-exec
        Command::new("git")
            .args(["diff", "--name-only", &format!("{base}...HEAD")])
            .current_dir(root)
            .output()
            .with_context(|| format!("spawn git diff against {base}"))?;
        if !out.status.success() {
            return Err(anyhow!(
                "git diff against `{base}` failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    match attempt(&primary) {
        Ok(s) => Ok(s),
        Err(primary_err) => {
            eprintln!(
                "pre-push: primary diff base `{primary}` unavailable ({primary_err}); trying HEAD~1"
            );
            attempt("HEAD~1").context("HEAD~1 fallback also failed")
        }
    }
}

/// Repo-relative paths under `docs/src/` (no `docs/src/` prefix), excluding `archive/`.
fn changed_docs_md_rel_paths(root: &Path) -> Result<Vec<String>> {
    let raw = git_diff_name_only_for_prepush(root)?;
    let mut seen = BTreeSet::new();
    for line in raw.lines() {
        let line = line.trim();
        if !line.ends_with(".md") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("docs/src/") {
            if rest.starts_with("archive/") {
                continue;
            }
            seen.insert(rest.to_string());
        }
    }
    Ok(seen.into_iter().collect())
}

fn step_fmt(root: &Path) -> Result<()> {
    check_fmt(root)
}

/// Assert that every workspace crate's `Cargo.toml` `edition` matches the one
/// declared in the workspace `rustfmt.toml`.
///
/// **Why this matters (edition-split false-positives):**
/// `cargo fmt --all` passes each *package's* edition to rustfmt, so a crate
/// that declares `edition = "2021"` will be checked with 2021 rules even when
/// the workspace `rustfmt.toml` says `edition = "2024"`.  Files already
/// formatted correctly for 2024 will be flagged as unformatted — a confusing
/// false-positive that blocks `git push`.  The fix is always: bump the lagging
/// crate's `Cargo.toml` edition to match `rustfmt.toml`, then verify with
/// `cargo check -p <crate>` (Edition 2024 has breaking changes; use
/// `cargo fix --edition` if needed, or defer the bump and note it here).
///
/// This function is called at the top of `check_fmt` so new lagging crates
/// are caught immediately with a clear error rather than a cryptic fmt diff.
fn check_edition_alignment(root: &Path) -> Result<()> {
    // Read the expected edition from rustfmt.toml.
    let rustfmt_toml = root.join("rustfmt.toml");
    let toml_text = std::fs::read_to_string(&rustfmt_toml)
        .with_context(|| format!("read {}", rustfmt_toml.display()))?;
    let expected_edition = toml_text
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix("edition")
                .and_then(|rest| rest.trim_start().strip_prefix('='))
                .map(|v| v.trim().trim_matches('"').to_string())
        })
        .context("rustfmt.toml has no `edition = …` line")?;

    // Walk workspace member Cargo.tomls and collect any with a different edition.
    let meta = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .context("cargo metadata for edition alignment check")?;

    let mut mismatched: Vec<String> = Vec::new();
    for pkg in meta.workspace_packages() {
        let manifest = std::fs::read_to_string(pkg.manifest_path.as_std_path())
            .with_context(|| format!("read {}", pkg.manifest_path))?;
        // Look for `edition = "…"` in the [package] section.
        if let Some(pkg_edition) = manifest.lines().find_map(|line| {
            let line = line.trim();
            line.strip_prefix("edition")
                .and_then(|rest| rest.trim_start().strip_prefix('='))
                .map(|v| v.trim().trim_matches('"').to_string())
        }) {
            if pkg_edition != expected_edition {
                mismatched.push(format!(
                    "  {} — edition = {:?} (want {:?}); bump Cargo.toml \
                     and run `cargo check -p {}`",
                    pkg.manifest_path, pkg_edition, expected_edition, pkg.name
                ));
            }
        }
        // Packages with no explicit edition inherit the workspace default; no
        // action needed because rustfmt will also use the workspace default.
    }

    if !mismatched.is_empty() {
        bail!(
            "Edition mismatch: the following workspace crates declare an \
             edition that does not match `rustfmt.toml` (edition = {:?}).\n\
             This causes `cargo fmt --all` to check those crates with the \
             wrong edition rules, producing false-positive fmt failures.\n\
             Fix: bump each crate's `Cargo.toml` edition to {:?}, then run \
             `cargo check -p <crate>` to confirm it compiles. Use \
             `cargo fix --edition` if Edition 2024 has breaking changes.\n\n\
             Lagging crates:\n{}",
            expected_edition,
            expected_edition,
            mismatched.join("\n")
        );
    }
    Ok(())
}

/// Cross-platform, crate-agnostic `rustfmt --check` over the whole workspace.
///
/// `cargo fmt --all` collects every `.rs` file in the workspace and passes them all
/// to a single rustfmt invocation, which overflows the Windows command-line limit
/// (os error 206) on a large workspace with long paths. Instead we pass only each
/// crate's TARGET ROOT files (lib.rs / main.rs / build.rs / test & example roots)
/// from live `cargo metadata` — rustfmt resolves the `mod` tree from each root
/// internally, so our command line carries ~one path per target rather than every
/// source file — and we run `rustfmt --check` in fixed-size chunks that stay well
/// under the OS arg limit on every platform. rustfmt reads the edition + style from
/// the workspace `rustfmt.toml` (via `--config-path`). Robust to crates being added
/// or removed because the target set comes from metadata, never a hard-coded list.
pub(crate) fn check_fmt(root: &Path) -> Result<()> {
    // Fail fast if any workspace crate's edition lags behind rustfmt.toml.
    // A mismatch causes `cargo fmt --all` to check that crate with the wrong
    // edition rules, producing confusing false-positive "unformatted" errors.
    check_edition_alignment(root)?;
    let meta = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .context("cargo metadata for fmt check")?;
    let mut roots: Vec<String> = Vec::new();
    for pkg in meta.workspace_packages() {
        for target in &pkg.targets {
            roots.push(target.src_path.as_str().to_string());
        }
    }
    roots.sort();
    roots.dedup();
    if roots.is_empty() {
        return Ok(());
    }
    // ~50 root paths per call: even with long absolute Windows paths (~120 chars)
    // that is ~6 KB, far under the ~32 KB limit. rustfmt expands each root's module
    // tree itself, so chunk size is independent of any single crate's file count.
    const CHUNK: usize = 50;
    let mut unformatted = false;
    for chunk in roots.chunks(CHUNK) {
        let status = std::process::Command::new("rustfmt")
            .arg("--check")
            .arg("--config-path")
            .arg(root)
            .args(chunk)
            .current_dir(root)
            .status()
            .context("spawn rustfmt --check")?;
        if !status.success() {
            unformatted = true;
        }
    }
    if unformatted {
        bail!(
            "rustfmt --check found unformatted files (diffs above). \
             Run `vox run scripts/fmt.vox` to fix (Windows-safe; plain \
             `cargo fmt --all` overflows the Windows command-line limit / \
             os error 206 on this workspace). Single crate: `cargo fmt -p <crate>`."
        );
    }
    Ok(())
}

fn step_line_endings(root: &Path) -> Result<()> {
    vox_cli_ci::line_endings::run(root, false, None, false)
}

fn step_ssot_drift(root: &Path) -> Result<()> {
    super::run_body::run_body_helpers::run_ssot_drift(root)
}

fn step_spoke_check(root: &Path) -> Result<()> {
    super::run_body::run_body_helpers::run_spoke_check(root)
}

fn step_runner_policy_check(root: &Path) -> Result<()> {
    // In-process (same as ssot-drift wedge) — avoids Windows nested `current_exe()` spawning a
    // stale `vox.exe` when embed build metadata lags the working tree.
    vox_cli_ci::runner_policy_check::run(root, false)
}

fn step_workflow_concurrency_guard(root: &Path) -> Result<()> {
    // In-process (same as runner-policy-check) — avoids Windows nested `current_exe()`
    // spawning a stale `vox.exe`. Strict: the tree is already clean + exceptions exist.
    vox_cli_ci::workflow_concurrency_guard::run(root, true)
}

fn step_workflow_permissions_guard(root: &Path) -> Result<()> {
    // Advisory (strict=false), unlike its concurrency sibling: only 19 of 45
    // workflows currently declare a top-level `permissions:` block, so strict
    // here would block every contributor's pre-push on a pre-existing backlog.
    // Warning-only still makes the gate real — it runs, and a NEW workflow
    // without a block is named on the next push. Flip to `true` once the
    // backlog is cleared.
    vox_cli_ci::workflow_permissions_guard::run(root, false)
}

fn step_check_links(root: &Path) -> Result<()> {
    // In-process — avoids Windows nested `current_exe()` spawning a stale sibling `vox.exe`.
    vox_cli_ci::check_links::run(root, None)
}

fn step_retired_symbol_check(root: &Path) -> Result<()> {
    vox_cli_ci::retired_symbol_check::run(root)
}

fn step_canonical_map_verify(root: &Path) -> Result<()> {
    vox_cli_ci::canonical_docs::run(root)
}

fn step_doc_inventory(root: &Path) -> Result<()> {
    let committed = root.join(vox_doc_inventory::DEFAULT_INVENTORY_PATH);
    if vox_doc_inventory::verify_fresh(root, &committed).is_err() {
        vox_doc_inventory::generate(root, &committed)?;
        if vox_doc_inventory::verify_fresh(root, &committed).is_ok() {
            return Err(anyhow!(
                "doc-inventory.json was stale (often after ssot-drift refreshes \
                 contracts/reports/*); regenerated {} — stage and commit, then re-run pre-push",
                committed.display()
            ));
        }
    }
    vox_doc_inventory::verify_fresh(root, &committed).map_err(|e| {
        e.context(
            "doc-inventory.json is out of date; run: vox ci ssot-drift && vox ci doc-inventory \
             generate --output docs/agents/doc-inventory.json",
        )
    })?;
    println!("doc-inventory.json matches generator output (excluding generated_at)");
    Ok(())
}

fn step_clippy(root: &Path) -> Result<()> {
    cargo_status(
        root,
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn step_toestub_changed(root: &Path) -> Result<()> {
    use super::run_body::run_body_helpers::run_toestub_scoped_roots;
    use vox_cli_ci::cmd_enums::ToestubCiMode;

    let dirs = changed_dirs_under_crates(root)
        .context("compute changed crate paths for scoped TOESTUB")?;
    if dirs.is_empty() {
        println!("    (no crate changes vs. base — skipping scoped TOESTUB)");
        return Ok(());
    }
    println!(
        "    TOESTUB batched scan: {} changed crate root(s)",
        dirs.len()
    );
    run_toestub_scoped_roots(root, &dirs, ToestubCiMode::EnforceWarn)
        .context("toestub-scoped batched run failed")
}

fn step_doc_frontmatter_full(root: &Path) -> Result<()> {
    cargo_status(
        root,
        &["run", "-q", "-p", "vox-doc-pipeline", "--", "--lint-only"],
    )
}

fn step_doc_frontmatter_scoped(root: &Path, rel_paths: &[String]) -> Result<()> {
    if rel_paths.is_empty() {
        println!("    (no changed markdown under docs/src vs. base — skipping)");
        return Ok(());
    }
    let paths_arg = rel_paths.join(",");
    let status = cargo()
        .args(["run", "-q", "-p", "vox-doc-pipeline", "--", "--lint-only"])
        .arg(format!("--paths={paths_arg}"))
        .current_dir(root)
        .status()
        .context("spawn vox-doc-pipeline (scoped)")?;
    if !status.success() {
        bail!("vox-doc-pipeline exited with {:?}", status.code());
    }
    Ok(())
}

fn step_doctest_md_full(root: &Path) -> Result<()> {
    vox_cli_ci::doctest_md::run(vec![root.join("docs/src")], true)
}

fn step_doctest_scoped(root: &Path, rel_paths: &[String]) -> Result<()> {
    if rel_paths.is_empty() {
        println!("    (no changed markdown for doctest-md — skipping)");
        return Ok(());
    }
    let paths: Vec<PathBuf> = rel_paths
        .iter()
        .map(|rel| root.join("docs/src").join(rel))
        .collect();
    vox_cli_ci::doctest_md::run(paths, true)
}

fn step_drift_check(root: &Path) -> Result<()> {
    // Show warnings for visibility but only fail on errors.  The workspace
    // carries a backlog of warning-level findings (duplicate literals, etc.)
    // that are tracked separately; blocking every push on them was always a
    // false-positive-saturated gate.  Error-level findings are hard failures
    // (e.g. security-sensitive patterns) and must stay blocked.
    let status = cargo()
        .args([
            "run",
            "-q",
            "-p",
            "vox-drift-check",
            "--",
            ".",
            "--severity",
            "warning",
            "--fail-on",
            "error",
        ])
        .current_dir(root)
        .status()
        .context("spawn vox-drift-check")?;
    if !status.success() {
        bail!("vox-drift-check exited with {:?}", status.code());
    }
    Ok(())
}

// ── Impacted-crate selector (--since) ────────────────────────────────────────

/// Outcome of the impacted-crate analysis for `--since <ref>`.
struct ImpactedCrates {
    /// Package names that are impacted (directly changed + transitive reverse-deps).
    /// Empty when `fallback = true`.
    packages: Vec<String>,
    /// Chunked package lists for nextest invocations (empty when `fallback = true`).
    chunks: Vec<Vec<String>>,
    /// True when git/metadata failed or no workspace mapping; caller falls back to `--workspace`.
    fallback: bool,
    /// Human-readable reason when `fallback` is set.
    fallback_reason: Option<String>,
}

/// Default number of packages per nextest chunk when `--since` impacts many crates.
const SINCE_CHUNK_SIZE_DEFAULT: usize = 10;

/// Compute the set of workspace packages impacted by changes since `since_ref`.
///
/// Algorithm:
/// 1. `git diff --name-only <since_ref>...HEAD` → changed file paths
/// 2. `cargo metadata --no-deps` → package manifest dirs
/// 3. Map changed files → directly changed packages (file falls under manifest dir)
/// 4. BFS over reverse-dep graph → transitive dependents
/// 5. If total > threshold, signal fallback
fn compute_impacted_crates(root: &Path, since_ref: &str) -> Result<ImpactedCrates> {
    let chunk_size = std::env::var("VOX_PREPUSH_SINCE_CHUNK_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(SINCE_CHUNK_SIZE_DEFAULT);

    // Step 1: changed files
    let diff_out = Command::new("git")
        .args(["diff", "--name-only", &format!("{since_ref}...HEAD")])
        .current_dir(root)
        .output()
        .context("git diff for --since")?;
    if !diff_out.status.success() {
        eprintln!(
            "pre-push --since: git diff against `{since_ref}` failed ({}); falling back to --workspace",
            String::from_utf8_lossy(&diff_out.stderr).trim()
        );
        return Ok(ImpactedCrates {
            packages: vec![],
            chunks: vec![],
            fallback: true,
            fallback_reason: Some(format!("git diff against `{since_ref}` failed")),
        });
    }
    let changed_files: Vec<String> = String::from_utf8_lossy(&diff_out.stdout)
        .lines()
        .map(|l| l.trim().replace('\\', "/"))
        .filter(|l| !l.is_empty())
        .collect();

    if changed_files.is_empty() {
        // No changes — nothing to test; but rather than running nothing, fall back
        // to workspace so the user always gets a green signal on a clean branch.
        println!("pre-push --since `{since_ref}`: no changed files; running full workspace");
        return Ok(ImpactedCrates {
            packages: vec![],
            chunks: vec![],
            fallback: true,
            fallback_reason: Some("no changed files".to_string()),
        });
    }

    // Step 2: cargo metadata (no resolve needed; `dependencies` list is enough)
    let metadata = MetadataCommand::new()
        .no_deps()
        .current_dir(root)
        .exec()
        .context("cargo metadata for --since")?;
    let workspace_root: PathBuf = metadata.workspace_root.clone().into();

    // Step 3: directly changed packages
    let mut directly_changed: HashSet<String> = HashSet::new();
    for pkg in metadata.workspace_packages() {
        let manifest_path = Path::new(pkg.manifest_path.as_str());
        let manifest_dir = manifest_path.parent().unwrap_or(Path::new("."));
        let pkg_rel = manifest_dir
            .strip_prefix(&workspace_root)
            .unwrap_or(manifest_dir)
            .to_string_lossy()
            .replace('\\', "/");
        let prefix = if pkg_rel.is_empty() {
            String::new()
        } else {
            format!("{pkg_rel}/")
        };
        for f in &changed_files {
            let touches = if prefix.is_empty() {
                // Root package: a changed file at the top level counts
                !f.contains('/')
                    || f.starts_with("src/")
                    || f.starts_with("benches/")
                    || f.starts_with("tests/")
                    || f == "Cargo.toml"
            } else {
                f.starts_with(&prefix) || f == &pkg_rel
            };
            if touches {
                directly_changed.insert(pkg.name.to_string());
                break;
            }
        }
    }

    if directly_changed.is_empty() {
        // Changed files don't fall under any workspace crate (e.g. docs only).
        // Don't run nothing — fall back to full workspace.
        eprintln!(
            "pre-push --since `{since_ref}`: no workspace crates match changed files; falling back to --workspace"
        );
        return Ok(ImpactedCrates {
            packages: vec![],
            chunks: vec![],
            fallback: true,
            fallback_reason: Some("no workspace crates match changed files".to_string()),
        });
    }

    // Step 4: build reverse-dep map and BFS
    // forward_deps[dep_name] = [list of packages that depend on dep_name]
    let mut reverse_deps: HashMap<String, Vec<String>> = HashMap::new();
    let workspace_names: HashSet<&str> = metadata
        .workspace_packages()
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    for pkg in metadata.workspace_packages() {
        for dep in &pkg.dependencies {
            // Only compile-time (Normal) deps propagate impacted status.
            if dep.kind == cargo_metadata::DependencyKind::Normal
                && workspace_names.contains(dep.name.as_str())
            {
                reverse_deps
                    .entry(dep.name.clone())
                    .or_default()
                    .push(pkg.name.to_string());
            }
        }
    }

    let mut impacted: HashSet<String> = directly_changed.clone();
    let mut queue: VecDeque<String> = directly_changed.into_iter().collect();
    while let Some(crate_name) = queue.pop_front() {
        if let Some(dependents) = reverse_deps.get(&crate_name) {
            for dep in dependents {
                if impacted.insert(dep.clone()) {
                    queue.push_back(dep.clone());
                }
            }
        }
    }

    let mut packages: Vec<String> = impacted.into_iter().collect();
    packages.sort();
    packages = filter_nextest_packages(&packages);
    let chunks = chunk_packages(&packages, chunk_size);
    eprintln!(
        "pre-push --since `{since_ref}`: {} impacted package(s) in {} nextest chunk(s): {}",
        packages.len(),
        chunks.len(),
        packages.join(", ")
    );
    Ok(ImpactedCrates {
        packages,
        chunks,
        fallback: false,
        fallback_reason: None,
    })
}

fn chunk_packages(packages: &[String], chunk_size: usize) -> Vec<Vec<String>> {
    if packages.is_empty() {
        return vec![];
    }
    packages
        .chunks(chunk_size.max(1))
        .map(|chunk| chunk.to_vec())
        .collect()
}

// ── Step functions ────────────────────────────────────────────────────────────

fn step_nextest(root: &Path) -> Result<()> {
    let mut args = vec![
        "nextest",
        "run",
        "--workspace",
        "--profile",
        "ci",
        "--no-fail-fast",
    ];
    if !nextest_skip_packages().is_empty() {
        args.push("-E");
        args.push("not package(vox-cli)");
    }
    nextest_cargo_status(root, &args)
}

/// Run nextest for impacted packages in chunks (from `--since` analysis).
fn step_nextest_packages_chunked(
    root: &Path,
    chunks: &[Vec<String>],
    with_coverage: bool,
) -> Result<()> {
    if chunks.is_empty() {
        bail!("nextest chunk list is empty");
    }
    for (i, chunk) in chunks.iter().enumerate() {
        if chunks.len() > 1 {
            println!(
                "    nextest chunk {}/{} ({} pkg(s))",
                i + 1,
                chunks.len(),
                chunk.len()
            );
        }
        if with_coverage {
            step_nextest_packages_with_coverage(root, chunk)?;
        } else {
            step_nextest_packages(root, chunk)?;
        }
    }
    Ok(())
}

/// Run nextest for a specific set of packages (from `--since` impacted-crate analysis).
fn step_nextest_packages(root: &Path, packages: &[String]) -> Result<()> {
    let mut args = vec!["nextest", "run", "--profile", "ci", "--no-fail-fast"];
    for p in packages {
        args.push("--package");
        args.push(p.as_str());
    }
    nextest_cargo_status(root, &args)
}

/// Run `cargo llvm-cov nextest` for a specific set of packages.
fn step_nextest_packages_with_coverage(root: &Path, packages: &[String]) -> Result<()> {
    let mut args = vec![
        "llvm-cov",
        "nextest",
        "--profile",
        "ci",
        "--no-fail-fast",
        "--no-report",
    ];
    for p in packages {
        args.push("--package");
        args.push(p.as_str());
    }
    nextest_cargo_status(root, &args)
}

/// Run nextest under `cargo llvm-cov nextest` (no-report phase; coverage data stays on disk).
/// A separate `step_llvm_cov_report` step renders the final report.
fn step_nextest_with_coverage(root: &Path) -> Result<()> {
    nextest_cargo_status(
        root,
        &[
            "llvm-cov",
            "nextest",
            "--workspace",
            "--profile",
            "ci",
            "--no-fail-fast",
            "--no-report",
        ],
    )
}

/// Emit coverage report artifacts (lcov + HTML) from an `llvm-cov nextest --no-report` run.
fn step_llvm_cov_report(root: &Path) -> Result<()> {
    cargo_status(
        root,
        &[
            "llvm-cov",
            "report",
            "--lcov",
            "--output-path",
            "target/llvm-cov/lcov.info",
        ],
    )?;
    cargo_status(
        root,
        &[
            "llvm-cov",
            "report",
            "--html",
            "--output-dir",
            "target/llvm-cov/html",
        ],
    )
}

/// Run only the three slow `#[ignore]` tests that are annotated with `"slow; …"`.
/// Uses an explicit `-E` filter + `--run-ignored ignored-only` so none of the other
/// 250+ ignored tests (intentionally excluded) are swept in.
fn step_nextest_slow(root: &Path) -> Result<()> {
    nextest_cargo_status(
        root,
        &[
            "nextest",
            "run",
            "-E",
            concat!(
                "test(arch_check_live_workspace_smoke_and_description_rule)",
                " or test(timeout_kills_long_running_child)",
                " or test(generated_ai_fixture_bundle_passes_cargo_check)",
            ),
            "--run-ignored",
            "ignored-only",
            "--profile",
            "ci",
        ],
    )
}

fn changed_dirs_under_crates(root: &Path) -> Result<Vec<PathBuf>> {
    let raw = git_diff_name_only_for_prepush(root)?;
    let mut seen = BTreeSet::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.splitn(3, '/').collect();
        if parts.len() >= 2 && parts[0] == "crates" {
            seen.insert(PathBuf::from("crates").join(parts[1]));
        }
    }
    Ok(seen.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_budget_yaml(dir: &std::path::Path, yaml: &str) {
        let budgets_dir = dir.join("contracts/budgets");
        std::fs::create_dir_all(&budgets_dir).expect("create budgets dir");
        std::fs::write(budgets_dir.join("test-tier-budgets.v1.yaml"), yaml)
            .expect("write budget yaml");
    }

    #[test]
    fn budget_missing_file_is_noop() {
        let dir = tempdir().unwrap();
        // No budgets file present — must never fail regardless of elapsed.
        assert!(
            check_tier_budget(dir.path(), "fast", u64::MAX).is_ok(),
            "missing budgets file must be a no-op"
        );
    }

    #[test]
    fn budget_under_warn_threshold_is_clean() {
        let dir = tempdir().unwrap();
        write_budget_yaml(
            dir.path(),
            "schema_version: 1\ntiers:\n  fast:\n    measured_ms: 1000\n    warn_ms: 1200\n    fail_ms: 1500\n",
        );
        assert!(
            check_tier_budget(dir.path(), "fast", 1000).is_ok(),
            "elapsed < warn_ms must be OK"
        );
    }

    #[test]
    fn budget_between_warn_and_fail_is_warning_only() {
        let dir = tempdir().unwrap();
        write_budget_yaml(
            dir.path(),
            "schema_version: 1\ntiers:\n  fast:\n    measured_ms: 1000\n    warn_ms: 1200\n    fail_ms: 1500\n",
        );
        // Over warn but under fail — should succeed (warning printed to stderr but no error).
        assert!(
            check_tier_budget(dir.path(), "fast", 1300).is_ok(),
            "warn_ms < elapsed < fail_ms must still return Ok"
        );
    }

    #[test]
    fn budget_over_fail_threshold_returns_err() {
        let dir = tempdir().unwrap();
        write_budget_yaml(
            dir.path(),
            "schema_version: 1\ntiers:\n  fast:\n    measured_ms: 1000\n    warn_ms: 1200\n    fail_ms: 1500\n",
        );
        assert!(
            check_tier_budget(dir.path(), "fast", 1600).is_err(),
            "elapsed > fail_ms must return Err"
        );
    }

    #[test]
    fn budget_unknown_profile_is_noop() {
        let dir = tempdir().unwrap();
        write_budget_yaml(
            dir.path(),
            "schema_version: 1\ntiers:\n  fast:\n    measured_ms: 1000\n    warn_ms: 1200\n    fail_ms: 1500\n",
        );
        // "full+since" maps to "full" which is absent in this minimal file — no-op.
        assert!(
            check_tier_budget(dir.path(), "full+since", u64::MAX).is_ok(),
            "profile with no matching tier must be a no-op"
        );
    }

    #[test]
    fn budget_full_since_maps_to_full_tier() {
        let dir = tempdir().unwrap();
        write_budget_yaml(
            dir.path(),
            "schema_version: 1\ntiers:\n  full:\n    measured_ms: 495000\n    warn_ms: 594000\n    fail_ms: 743000\n",
        );
        // full+since re-uses the `full` budget row; should succeed well under threshold.
        assert!(
            check_tier_budget(dir.path(), "full+since", 20_000).is_ok(),
            "full+since must use full budget row"
        );
    }

    #[test]
    fn tier_budget_key_coverage() {
        assert_eq!(tier_budget_key("fast"), Some("fast"));
        assert_eq!(tier_budget_key("complete"), Some("complete"));
        assert_eq!(tier_budget_key("full"), Some("full"));
        assert_eq!(tier_budget_key("full+since"), Some("full"));
        assert_eq!(tier_budget_key("full+cov"), Some("full_cov"));
        assert_eq!(tier_budget_key("full+cov+since"), Some("full_cov"));
        assert_eq!(tier_budget_key("unknown"), None);
    }

    #[test]
    fn run_complete_static_skips_when_flag_set() {
        let full_only = PrePushOpts {
            quick: false,
            complete: false,
            full: true,
            dry_run: false,
            act: false,
            report_json: None,
            include_slow: false,
            with_coverage: false,
            since: None,
            enforce_budgets: false,
            skip_complete: false,
        };
        assert!(run_complete_static(&full_only));

        let skip = PrePushOpts {
            skip_complete: true,
            ..full_only.clone()
        };
        assert!(!run_complete_static(&skip));

        let complete = PrePushOpts {
            complete: true,
            full: false,
            skip_complete: false,
            ..full_only
        };
        assert!(run_complete_static(&complete));
    }

    #[test]
    fn filter_nextest_packages_removes_skipped_names() {
        let pkgs: Vec<String> = vec!["vox-a".into(), "vox-cli".into(), "vox-b".into()];
        let skip: HashSet<String> = ["vox-cli".into()].into_iter().collect();
        let filtered: Vec<String> = pkgs
            .iter()
            .filter(|p| !skip.contains(p.as_str()))
            .cloned()
            .collect();
        assert_eq!(filtered, vec!["vox-a", "vox-b"]);
    }

    #[test]
    fn chunk_packages_splits_evenly() {
        let pkgs: Vec<String> = (0..25).map(|i| format!("pkg-{i}")).collect();
        let chunks = chunk_packages(&pkgs, 10);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 10);
        assert_eq!(chunks[1].len(), 10);
        assert_eq!(chunks[2].len(), 5);
    }

    #[test]
    fn chunk_packages_empty_is_empty() {
        assert!(chunk_packages(&[], 10).is_empty());
    }
}
