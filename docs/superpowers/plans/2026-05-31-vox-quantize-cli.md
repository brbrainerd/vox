# vox quantize CLI (SP-4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a top-level `vox quantize` CLI subcommand in `vox-ml-cli` that drives the `vox-quantize` engine and reports the size/quality trade-off.

**Architecture:** A thin command module that parses flags, maps `--to` to `QuantMixture`, calls `vox_quantize::quantize`, and renders the `QuantReport` as a human table or JSON.

**Tech Stack:** Rust, `clap` (existing in `vox-ml-cli`), `vox-quantize` (SP-1), `serde_json`.

**Spec:** `docs/superpowers/specs/2026-05-31-vox-quantize-cli-design.md`
**Depends on:** SP-1 (`vox-quantize` must be built and its public API stable).

---

### Task 1: Wire the dependency + command skeleton

**Files:**
- Modify: `crates/vox-ml-cli/Cargo.toml`
- Create: `crates/vox-ml-cli/src/commands/quantize.rs`
- Modify: `crates/vox-ml-cli/src/commands/mod.rs` (register module — match existing pattern)

- [ ] **Step 1: Add the dep**

In `crates/vox-ml-cli/Cargo.toml` `[dependencies]`:
```toml
vox-quantize = { workspace = true }
```

- [ ] **Step 2: Create the command module with arg parsing**

```rust
//! `vox quantize` — quantize a local SafeTensors model with vox-quantize.
use std::path::PathBuf;
use vox_quantize::{quantize, QuantMixture};

#[derive(Debug, clap::Args)]
pub struct QuantizeArgs {
    /// Model directory (must contain config.json + *.safetensors).
    #[arg(long)]
    pub input: PathBuf,
    /// Output directory for the quantized artifact.
    #[arg(long)]
    pub output: PathBuf,
    /// Target mixture: q4_k_m | q5_k_m | q6_k | q8_0
    #[arg(long, default_value = "q4_k_m")]
    pub to: String,
    /// Skip the round-trip verification pass.
    #[arg(long, default_value_t = false)]
    pub no_verify: bool,
    /// Emit the full report as JSON instead of a table.
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

pub fn parse_mixture(s: &str) -> anyhow::Result<QuantMixture> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "q4_k_m" => QuantMixture::Q4KM,
        "q5_k_m" => QuantMixture::Q5KM,
        "q6_k" => QuantMixture::Q6K,
        "q8_0" => QuantMixture::Q8_0,
        other => anyhow::bail!("unknown --to mixture `{other}` (expected q4_k_m|q5_k_m|q6_k|q8_0)"),
    })
}
```

- [ ] **Step 3: Register the module**

In `crates/vox-ml-cli/src/commands/mod.rs`, add `pub mod quantize;` alongside the existing module declarations.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p vox-ml-cli`
Expected: builds.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-ml-cli/Cargo.toml crates/vox-ml-cli/src/commands/quantize.rs crates/vox-ml-cli/src/commands/mod.rs
git commit -m "feat(ml-cli): scaffold vox quantize command + mixture parsing"
```

---

### Task 2: Mixture parsing test + run logic

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/quantize.rs`
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_mixture_maps_known_values() {
        assert!(matches!(parse_mixture("q4_k_m").unwrap(), QuantMixture::Q4KM));
        assert!(matches!(parse_mixture("Q8_0").unwrap(), QuantMixture::Q8_0));
        assert!(parse_mixture("bogus").is_err());
    }
}
```

- [ ] **Step 2: Run to verify it fails (or passes if Task 1 already covers)**

Run: `cargo test -p vox-ml-cli quantize::tests::parse_mixture_maps_known_values`
Expected: PASS (logic from Task 1) — if it fails, fix `parse_mixture`.

- [ ] **Step 3: Implement the run function + report rendering**

```rust
pub fn run(args: QuantizeArgs) -> anyhow::Result<()> {
    if !args.input.join("config.json").exists() {
        anyhow::bail!("no config.json in {} — not a model directory", args.input.display());
    }
    let mixture = parse_mixture(&args.to)?;
    let req = vox_quantize::engine::QuantizeRequest {
        input_dir: args.input.clone(),
        output_dir: args.output.clone(),
        mixture,
        verify: !args.no_verify,
    };
    let report = quantize(&req)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!("{:<60} {:>8} {:>12} {:>10}", "tensor", "dtype", "params", "mse");
    for s in &report.tensors {
        let note = if s.fallback { " (fallback)" } else { "" };
        println!("{:<60} {:>8} {:>12} {:>10.2e}{}", s.name, s.target_dtype, s.params, s.mse, note);
    }
    let gib = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);
    println!(
        "\n{:.2} GiB -> {:.2} GiB  ({:.2}x)   worst MSE {:.2e}",
        gib(report.total_src_bytes), gib(report.total_quant_bytes),
        report.compression_ratio, report.worst_mse,
    );
    Ok(())
}
```

> Note: this uses `vox_quantize::engine::QuantizeRequest`. Ensure SP-1 re-exports `QuantizeRequest` from the crate root (`pub use engine::QuantizeRequest;`); if it does, simplify to `vox_quantize::QuantizeRequest`. Add that re-export to SP-1 `lib.rs` if missing.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-ml-cli quantize::`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-ml-cli/src/commands/quantize.rs
git commit -m "feat(ml-cli): vox quantize run logic + report rendering"
```

---

### Task 3: Hook into the CLI command dispatch

**Files:**
- Modify: the `vox-ml-cli` top-level command enum (find it: `grep -rn "enum.*Command" crates/vox-ml-cli/src` — match where `Schola`/`Mens` variants are dispatched).

- [ ] **Step 1: Add the enum variant**

Add a `Quantize(commands::quantize::QuantizeArgs)` variant to the top-level subcommand enum, following the exact pattern of the neighbouring variant (e.g. how `MergeQlora`/`Schola` is wired).

- [ ] **Step 2: Dispatch it**

In the match arm that dispatches subcommands, add:
```rust
Command::Quantize(args) => commands::quantize::run(args),
```
matching the surrounding arms' return-type/`?` conventions.

- [ ] **Step 3: Manual smoke (end-to-end against SP-1 fixture)**

Run: `cargo run -p vox-ml-cli -- quantize --help`
Expected: help text lists `--input --output --to --no-verify --json`.

- [ ] **Step 4: Integration test**

**Files:** Test: `crates/vox-ml-cli/tests/quantize_cli.rs`

```rust
// Build a tiny model dir, run the command via the run() entrypoint, assert artifact.
#[test]
fn quantize_cli_produces_artifact() {
    use candle_core::{Device, DType, Tensor};
    use std::collections::HashMap;
    let indir = tempfile::tempdir().unwrap();
    let outdir = tempfile::tempdir().unwrap();
    let dev = Device::Cpu;
    let mut t: HashMap<String, Tensor> = HashMap::new();
    t.insert("model.language_model.layers.0.mlp.gate_proj.weight".into(),
        Tensor::randn(0f32, 1f32, (256, 256), &dev).unwrap());
    t.insert("model.language_model.norm.weight".into(),
        Tensor::ones((256,), DType::F32, &dev).unwrap());
    candle_core::safetensors::save(&t, indir.path().join("model.safetensors")).unwrap();
    std::fs::write(indir.path().join("config.json"),
        r#"{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],"hidden_size":256,"num_attention_heads":8,"num_hidden_layers":1,"vocab_size":512}"#).unwrap();

    let args = vox_ml_cli::commands::quantize::QuantizeArgs {
        input: indir.path().to_path_buf(),
        output: outdir.path().to_path_buf(),
        to: "q4_k_m".into(),
        no_verify: false,
        json: true,
    };
    vox_ml_cli::commands::quantize::run(args).unwrap();
    assert!(outdir.path().join("quant-metadata.json").exists());
}
```

> If `vox-ml-cli` is a binary-only crate without a `lib.rs`, expose `commands` via a `pub` lib target or test through `assert_cmd` invoking the built binary instead. Confirm the crate's lib/bin shape first (`cat crates/vox-ml-cli/Cargo.toml`).

- [ ] **Step 5: Run + commit**

Run: `cargo test -p vox-ml-cli --test quantize_cli`
Expected: PASS
```bash
git add crates/vox-ml-cli
git commit -m "feat(ml-cli): wire vox quantize into command dispatch + integration test"
```

---

## Self-Review

- **Spec coverage:** subcommand ✓ T1/T3; `--to` mixture mapping ✓ T1/T2; human + `--json` report ✓ T2; missing-config error ✓ T2; integration ✓ T3.
- **Placeholder scan:** none. Two conditional notes (crate-root re-export; lib-vs-bin shape) are explicit confirm-then-adjust instructions, not placeholders.
- **Type consistency:** `QuantizeArgs`/`parse_mixture`/`run` consistent across tasks; `QuantizeRequest`/`QuantMixture`/`QuantReport` match SP-1.
