# B8 Training Gates

This document describes the human-gated steps required to execute actual
training runs.  The code scaffolding (B8.1 pre-flight, B8.4 parity report)
is already implemented and tested.  The commands below require specific
hardware, an explicit spend-authorisation environment variable, and a human
go-ahead before they are executed.

---

## Pre-flight (always run first)

Before attempting any training, confirm the data-sufficiency spike shows at
least one PROCEED spoke:

```
vox mens preflight --spike mens/data-sufficiency-spike-b2_5.json
```

Per the current spike (`mens/data-sufficiency-spike-b2_5.json`):

| Spoke               | Rows | Diversity | Decision   |
|---------------------|------|-----------|------------|
| vox-lang            | 1200 | 0.55      | **PROCEED** |
| rust                |  900 | 0.48      | **PROCEED** |
| tool-selection      |  450 | 0.42      | BLOCKED (<500 rows) |
| argument-generation |  450 | 0.40      | BLOCKED (<500 rows) |

---

## Capture the base baseline (run BEFORE training a spoke)

The beat-base eval gate compares a trained adapter's BFCL accuracy against the
**base model** (no adapter). That comparison can only run if a baseline has been
captured first. Without `baseline_report.json` in the run dir, beat-base
**silently skips** and a below-baseline adapter could pass the gate.

For each spoke, before training:

1. Run the BFCL eval harness against the BASE model (no adapter) to produce a
   `bfcl_results.json` (`{"accuracy": <f64>, "total": <usize>}`) in a base-eval
   directory.
2. Capture it into the spoke's training run dir:

```
vox mens baseline \
  --spoke <name> \
  --base-eval-dir <base-eval-dir> \
  --out <train-run-dir>/baseline_report.json
```

Example:

```
vox mens baseline --spoke vox-lang \
  --base-eval-dir runs/vox-lang-base-eval/ \
  --out runs/vox-lang-20260622/baseline_report.json
```

This writes a `baseline_report.json` with a single `bfcl_accuracy` entry whose
confidence interval is a Wilson 95% score interval over `(accuracy, total)`.
The post-training `eval-gate` reads this file and runs the beat-base comparison.
The baseline MUST come from a real base eval — `vox mens baseline` fails closed
if `bfcl_results.json` is absent; it never fabricates a passing baseline.

---

## B8.2 — Smoke run (single spoke, harness-first)

### Local path (RTX 4080 SUPER required)

```
vox mens train --cloud local --spoke harness
```

**Hardware gate:** requires a local NVIDIA RTX 4080 SUPER (16 GB VRAM) or
equivalent.  The `qwen_4080_16g` preset is selected by default.  Running on
CPU-only hardware will fall back to the `dev_cpu` preset (slow; for smoke
verification only — not a production training run).

### Cloud path (RunPod spot)

```
VOX_MENS_ALLOW_SPEND=1 vox mens train --cloud runpod --spoke harness --apply
```

**Spend gate:** `VOX_MENS_ALLOW_SPEND=1` must be set in the calling shell.
The executor (this agent or any automation) **cannot** set this variable.
It is a human gate — only a human operator sets it before running the command.
`--apply` is also required; without it the command performs a dry-run only.

---

## B8.3 — Fan-out (ready spokes)

Run the two PROCEED spokes (`vox-lang`, `rust`).  `tool-selection` and
`argument-generation` are **BLOCKED** per the current B2.5 spike and must
not be included until their row counts are raised above 500.

```
VOX_MENS_ALLOW_SPEND=1 vox mens train --cloud runpod --spoke vox-lang,rust --apply
```

Same spend + apply gates as B8.2 cloud.

---

## Post-training evaluation

After each training run completes, check the beat-base gate. The gate reads the
run dir (which must contain `baseline_report.json` from the capture step above,
plus the trained adapter's `bfcl_results.json`):

```
vox mens eval-gate --run-dir <path-to-run-dir> [--policy <eval-gates.yaml>]
```

Examples:

```
vox mens eval-gate --run-dir runs/vox-lang-20260622/
vox mens eval-gate --run-dir runs/rust-20260622/ --policy mens/config/eval-gates-rust.yaml
```

> Note: `eval-gate` keys off the run dir, not a `--spoke` flag — the spoke is read
> from the run's `training_manifest.json`. The default policy is
> `mens/config/eval-gates.yaml`; pass `--policy` to use a spoke-specific gate.

Then generate the parity report (compares each trained adapter vs its baseline and
records Flash/Sonnet gaps as the north-star metric). The parity report is currently
a **library API** (`commands::mens::parity_report`, B8.4) — there is no
`vox mens parity-report` CLI subcommand yet; it is produced programmatically by the
fan-out orchestration, or wire a thin CLI arm when needed:

```
// commands::mens::parity_report
let entry = compute_parity_entry(spoke, trained_metric, &baseline_entry, flash_ref, sonnet_ref);
write_parity_report(Path::new("mens/parity-report-v1.json"), &report)?;
```

V1 is accepted when every spoke entry in the parity report has `beats_base: true`.
Flash/Sonnet parity is a north-star metric — it is **not** a pass/fail gate for V1.

---

## What the executor cannot do

- Set `VOX_MENS_ALLOW_SPEND=1` — this env var must be set by a human in
  the calling shell before invoking the spend-gated commands.
- Provision RunPod or any other cloud GPU resource without `--apply`.
- Lower the data-sufficiency thresholds without updating the spike report and
  re-running `vox corpus readiness`.

---

## Blocked spokes — remediation

To unblock `tool-selection` and `argument-generation`:

```
# Merge the two spokes into a combined agentic spoke (recommended):
vox corpus generate --config mens/config/mix-agentic.yaml
vox corpus readiness --spoke agentic --input corpus-agentic.jsonl

# Or: run full synthetic generation to reach ~1500 augmented rows:
vox corpus generate
vox corpus readiness --spoke tool-selection --input corpus-tool-selection.jsonl
vox corpus readiness --spoke argument-generation --input corpus-argument-generation.jsonl
```

After rows exceed 500 (per spoke) AND diversity exceeds 0.30, re-run the B2.5
data-sufficiency spike to update `mens/data-sufficiency-spike-b2_5.json`, then
re-run pre-flight.
