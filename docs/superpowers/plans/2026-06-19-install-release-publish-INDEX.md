# Install / Release / Publish — Program Plan Index

> Design: [`docs/superpowers/specs/2026-06-19-single-command-install-release-publish-design.md`](../specs/2026-06-19-single-command-install-release-publish-design.md)
> Sessions: brainstorm ✅ → plan (this) → critique → Gemini-Flash handoff.

Each track is a standalone plan producing working, testable software. **Track 0
is the prerequisite for all others** (they read its SSOT). Within each track,
every capability lands with its enforcing CI gate (locked decision Q5).

| Track | Plan file | Status | Depends on |
|-------|-----------|--------|------------|
| 0 — Distribution SSOT | [`2026-06-19-track0-distribution-ssot.md`](2026-06-19-track0-distribution-ssot.md) | ✅ written | — |
| A — Tiered install + enable | [`2026-06-19-track-a-tiered-install.md`](2026-06-19-track-a-tiered-install.md) | ✅ executed (Sonnet 4.6) | Track 0 |
| B — Release + nightly automation | [`2026-06-19-track-b-release-nightly-automation.md`](2026-06-19-track-b-release-nightly-automation.md) | ✅ written (Flash handoff ready) | Track 0 |
| C — crates.io publish readiness | _to write_ | pending | Track 0 |
| D — Supply-chain trust (plugin sig + SBOM) | _to write_ | pending | Track 0 |

## Track scope summaries (for the next planning passes)

**Track A — Tiered install + enable.** `voxup install <minimal|default|full>`
reads `profiles.v1.yaml`; PATH automation across bash/zsh/fish/PowerShell;
`vox doctor` becomes the per-tier dependency surfacer with opt-in `--fix`
(auto-provision never default); commit the prototyped `vox-langtool` minimal
binary. Gates: per-tier dep-closure parity; clean-machine E2E smoke per OS
(the cross-platform required smoke lane is threaded in here).

**Track B — Release + nightly automation.** Build `release-nightly.yml`
(green-main gate before publish, rolling `nightly` release) per
[`2026-06-17-nightly-release-pipeline-design.md`](../specs/2026-06-17-nightly-release-pipeline-design.md);
unify the release matrix on the SSOT `binaries`; add update notification (CLI
footer + GUI `tauri-plugin-updater`/toast). Gates: nightly-green-before-release;
release-artifact ↔ SSOT-binaries parity.

**Track C — crates.io publish readiness.** Per-crate `cargo publish --dry-run`
green with full metadata (description/license/repository/readme); leaf-first
publish order from the SSOT; hakari/workspace-hack handled so closures publish;
`publish.enabled` flip gated behind a readiness check (stays `false`). Gates:
publish-set dry-run parity; metadata completeness; no-cycle/leaf-order check.

> ⚠️ **Reconcile, don't duplicate.** A prior program already designed the
> crates.io *publish machinery* — see `project_gamify_gui_pluginization_plan_2026_06_18`
> (its **TrackB**: hakari-aware publish, workspace-hack published first;
> **R18** publishability arch-check gate; leaf-first → Clavis; human-gated).
> Track C here owns only the *publish-set data* (the SSOT `publish` block) and
> the dry-run/metadata gates that read it. Wire to that program's machinery
> rather than reinventing `cargo publish` orchestration. Audit it first.

**Track D — Supply-chain trust.** Plugin `sha256`/`signature` + `source_commit`
in `Plugin.toml`; verify on install and at dlopen load (close the RCE surface);
Linux GPG signing + SBOM + provenance-on-release. Gates: unsigned-plugin-load
fails closed; release SBOM present.

## Suggested execution order

0 → (A ∥ C) → B → D. A and C are largely independent once the SSOT exists; B
consumes A's binary tiers and B's nightly feeds D's signing surface.
