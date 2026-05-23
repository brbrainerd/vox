---
title: "How-To: Publish Scientia findings"
description: "Prepare, approve, and submit scientific findings from Vox Scientia using the publication manifest SSOT."
category: "How-To Guides"
last_updated: "2026-03-25"
training_eligible: true

schema_type: "HowTo"
---

# How-To: Publish Scientia findings

This workflow uses a single publication manifest in Codex (`publication_manifests`) with digest-bound approvals and scholarly submission tracking.

> Note: scholarly submit defaults to `local_ledger` (`VOX_SCHOLARLY_ADAPTER`). For architecture and lingo, see [VoxGiantia publication architecture](../archive/research-2026-q1/voxgiantia-publication-architecture.md). For operator inputs vs derived fields, see [operator inputs](scientia-publication-operator-inputs.md). For remediation, see [publication playbook](../reference/scientia-publication-playbook.md). Policy SSOT: [scientia-publication-automation-ssot](../archive/research-2026-q1/scientia-publication-automation-ssot.md), [worthiness rules](../reference/scientia-publication-worthiness-rules.md), [readiness audit](../archive/research-2026-q1/scientia-publication-readiness-audit.md).

## Fastest safe path

When you already have a prepared SCIENTIA manifest, the shortest safe default path is:

1. `vox scientia publication-preflight --publication-id <id> --with-worthiness`
2. Fix anything in `findings`, `manual_required`, and ordered `next_actions`.
3. Record two digest-bound approvals.
4. Run `vox scientia publication-scholarly-pipeline-run --publication-id <id> --dry-run`.
5. Re-run without `--dry-run` when the output looks correct.

Use `vox scientia publication-status --publication-id <id> --with-worthiness` as the ongoing checklist surface when you also want the worthiness rubric inline; without the flag it still includes the same readiness report and `next_actions`, plus approvals, attempts, submissions, and status events.

### Discovery → draft assistance (deterministic)

- `vox scientia publication-discovery-scan` — ranks stored `scientia` manifests by structured `scientia_evidence` signals (strong / supporting / informational). Use `vox db publication-discovery-scan` with `--content-type` / `--state` when you need filters beyond the scientia facade default.
- `vox scientia publication-discovery-explain --publication-id <id>` — machine explanation, manifest completion report, evidence completeness, and a **non-authoritative** transform preview (labels `machine_suggested` + `requires_human_review`).
- `vox scientia publication-transform-preview --publication-id <id>` — preview-only JSON for scholarly/social stubs.
- `vox scientia publication-discovery-refresh-evidence --publication-id <id>` — merges live Socrates telemetry + JSON sidecars, rebuilds `scientia_evidence` (headings, signals), upserts digest; emits `discovery_evidence_refreshed`. MCP: `vox_scientia_publication_discovery_refresh_evidence`.
- Preflight JSON now includes `destination_readiness` (credential **presence** checks; no secret values).

**Anti-slop:** LLM assists (`vox_scientia_assist_suggestions` in MCP) must output JSON checklists grounded on provided evidence; they do **not** establish novelty or scientific truth. See `contracts/scientia/machine-suggestion-block.schema.json` and [scientia-a2a-evidence-tasks](../archive/research-2026-q1/scientia-a2a-evidence-tasks.md).

## 1) Prepare a manifest

```bash
vox scientia publication-prepare \
  --publication-id ai-research-2026-03 \
  --author "Your Name" \
  docs/src/research/ai-research-2026-03.md
```

If you omit `--title`, Vox now infers it from markdown frontmatter `title:` or the first `# Heading`.

Optional: pass `--title`, `--abstract-text`, `--citations-json <file>`, and `--scholarly-metadata-json <file>` (structured JSON for `scientific_publication`: authors with optional ORCID/affiliation, `license_spdx`, `funding_statement`, `competing_interests_statement`, `reproducibility`, `ethics_and_impact` — see `vox_publisher::scientific_metadata`). The same `--scholarly-metadata-json` flag works on `vox db publication-prepare`.

To use `publication-prepare` as an early discovery-to-draft bridge instead of a blank manifest step, also pass any structured evidence you already have:

- `--eval-gate-report-json <repo-file>`
- `--benchmark-pair-report-json <repo-file>`
- `--human-meaningful-advance`
- `--human-ai-disclosure-complete`

When those inputs are present, SCIENTIA seeds `metadata_json.scientia_evidence` with discovery signals, draft-preparation hints, and a short candidate note, then records a `discovery_candidate_prepared` status event.

Use `--preflight` (or `publication-prepare-validated`) -> run `vox_publisher::publication_preflight` before persisting; use `--preflight-profile arxiv-assist` when the handoff target is arXiv (requires `abstract_text`). Optional `--discovery-intake-gate strong-signals-only` or `allow-review-suggested` blocks scientia `publication-prepare` when deterministic discovery rank does not meet the tier (empty evidence ranks as low-signal unless you pass sidecars). MCP `vox_scientia_publication_prepare` accepts `scientia_evidence` JSON and the same gate when you prepare from agents without repo-relative report files. Use `publication-preflight` to inspect readiness JSON for an existing id (including `manual_required`, `confidence`, and live-publish gate hints when VoxDb is attached); add `--with-worthiness` to score against `contracts/scientia/publication-worthiness.default.yaml`. CLI-prepared manifests now include `repository_id` automatically, so `--with-worthiness` can merge live `socrates_surface` telemetry and repo-local `scientia_evidence` sidecars into the same decision path. You may also embed `scientia_evidence` manually (eval-gate result, baseline/candidate run ids, `human_meaningful_advance`, `human_ai_disclosure_complete`) so worthiness blends orchestrator telemetry with explicit human attestations. Use `publication-zenodo-metadata` to emit a Zenodo `metadata` object (stdout) for manual or scripted upload.

## 2) Record approvals (two distinct approvers)

```bash
vox scientia publication-approve --publication-id ai-research-2026-03 --approver alice
vox scientia publication-approve --publication-id ai-research-2026-03 --approver bob
```

Approvals are bound to the current content digest. If content changes, re-approve the new digest.

## 3) Default scholarly pipeline

```bash
vox scientia publication-scholarly-pipeline-run --publication-id ai-research-2026-03 --dry-run
vox scientia publication-scholarly-pipeline-run --publication-id ai-research-2026-03
```

This is the preferred scholarly path because it reuses preflight, the dual-approval gate, optional staging export, and submit in one flow instead of asking the operator to choose the low-level sequence each time.

## 4) Submit to scholarly adapter directly

```bash
vox scientia publication-submit-local --publication-id ai-research-2026-03
```

`publication-submit-local` uses the scholarly adapter selected by `VOX_SCHOLARLY_ADAPTER` (default `local_ledger`; `echo_ledger` for deterministic/no-network tests) and writes submission metadata to `scholarly_submissions`. Unknown adapter names **error** (no silent fallback).

## 5) Inspect lifecycle state

```bash
vox scientia publication-status --publication-id ai-research-2026-03 --with-worthiness
```

The status payload includes:

- current manifest state
- active content digest + version
- approval count for that digest
- embedded preflight report with `manual_required` and ordered `next_actions`
- optional inline worthiness output when `--with-worthiness` is set
- scholarly submission rows and external submission ids
- media assets, publication attempt timeline, and status event timeline

## 6) Optional social distribution metadata

To drive Reddit/Hacker News/YouTube planning from the same manifest, embed a
**`metadata_json.syndication`** object conforming to:

- `contracts/scientia/distribution.schema.json`
- `contracts/scientia/distribution.default.yaml`

Legacy manifests may still use **`metadata_json.scientia_distribution`**. At hydrate time the publisher **deep-merges** legacy + canonical keys (canonical `syndication` wins on conflicts), normalizes contract `channels` / `channel_payloads` into the flat runtime shape, and logs a deprecation warning when the legacy root is present. `vox db publication-preflight` surfaces the same hint under `manual_required`.

Important runtime alignment notes:

- `distribution_policy.channel_policy` is the supported location for per-channel policy.
- Root-level `channel_policy` is deprecated; runtime migrates it with a warning.
- `crosspost_plan` is currently reserved and ignored by runtime hydration.
- Channels like `reddit`, `github`, `open_collective`, `youtube`, and `crates_io` need matching `channel_payloads.<channel>` blocks before they materialize into a live runtime channel.

Optional **`metadata_json.topic_pack`**: set to a pack id from `contracts/scientia/distribution.topic-packs.yaml` (for example `research_breakthrough`). At hydrate time the pack **merges** worthiness floors, template profiles, and topic filters into the effective syndication config. **Channel allowlists** in the pack **drop** any channel not listed for that pack (after merge), so operators can tighten routing without editing every manifest.

**Minimum-input recipe:** set `topic_pack` + enable only the channels you need (or rely on pack allowlists). Omit per-channel payloads when the pack supplies policy; add `channel_payloads` / flat `twitter` / `reddit` blocks only for overrides.

Example skeleton:

```json
{
  "topic_pack": "research_breakthrough",
  "syndication": {
    "channels": ["reddit", "hacker_news", "youtube"],
    "channel_payloads": {
      "reddit": {
        "subreddit": "MachineLearning",
        "kind": "link"
      },
      "hacker_news": {
        "mode": "manual_assist"
      },
      "youtube": {
        "video_asset_ref": "artifacts/videos/demo.mp4",
        "privacy_status": "private"
      }
    },
    "distribution_policy": {
      "approval_required": true,
      "dry_run": true,
      "channel_policy": {
        "reddit": {
          "enabled": true,
          "template_profile": "deep_dive_selfpost",
          "worthiness_floor": 0.82,
          "topic_filters": {
            "include_tags": ["research_breakthrough", "benchmark"],
            "exclude_tags": ["internal_only"],
            "min_topic_score": 0.2
          }
        }
      }
    }
  }
}
```

Notes:

- Hacker News support is manual-assist only (official API is read-only).
- YouTube support uses OAuth refresh + resumable upload and should remain policy-gated by quota and audit readiness.
- `crates_io` is modeled in routing policy and outcomes; live publish adapter wiring remains intentionally explicit (non-implicit).
- `distribution_policy.channel_policy.*.template_profile` **does not change copy** unless `VOX_SYNDICATION_TEMPLATE_PROFILE=1` / `true` (then Twitter/Reddit/YouTube derived text caps follow named profiles such as `brief` / `roomy`; see `docs/src/reference/env-vars.md`).
- Configure social credentials via `VOX_SOCIAL_*` environment variables (`docs/src/reference/env-vars.md`).
- SSOT precedence is: manifest overrides > distribution policy defaults/contracts > runtime env overrides.

## 7) Route simulation and controlled fan-out

Use `vox db` for operator controls that are broader than the `vox scientia` convenience subset:

```bash
vox db publication-route-simulate --publication-id ai-research-2026-03
vox db publication-route-simulate --publication-id ai-research-2026-03 --json
vox db publication-publish --publication-id ai-research-2026-03 --channels reddit,youtube --dry-run true
vox db publication-publish --publication-id ai-research-2026-03 --channels reddit,youtube --dry-run true --json
vox db publication-retry-failed --publication-id ai-research-2026-03 --dry-run true
vox db publication-retry-failed --publication-id ai-research-2026-03 --dry-run true --json
```

Add `--json` for machine-readable stdout (one structured object per invocation). MCP equivalents `vox_scientia_publication_publish` and `vox_scientia_publication_retry_failed` accept **`json: true`** for a single-line compact JSON tool envelope.

**Retry-failed idempotency:** `publication-retry-failed` / MCP `vox_scientia_publication_retry_failed` pick candidates from the latest **digest-bound** attempt. Channels that already have a `Success` outcome for that digest are **not** republished (they appear as `skipped_success_channels`). Explicit `--channel` / `channel` follows the same planner so operators cannot accidentally duplicate a succeeded post when retrying a subset.


