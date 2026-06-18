# Dynamic VRAM Auditing & Model Estimation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a dynamic VRAM auditing system that queries active GPU memory usage via `nvidia-smi`, adjusts hyperparameter presets (sequence length, batch size) dynamically to prevent OOMs, and auto-enables gradient checkpointing for large models.

**Architecture:** We will parse `nvidia-smi` output to determine real-time VRAM availability. We will update the memory budget calculations to dynamically scale the resident footprint based on quantization mode, model family (Qwen 2.5, 3, 3.5), and gradient checkpointing. Finally, we will bound hyperparameter preset defaults to guarantee they fit in the audited available memory.

**Tech Stack:** Rust, Command API (processes), Cargo.

---

## File Structure

The following files will be modified:
1. `crates/vox-populi/src/mens/tensor/vram_autodetect.rs`: Add parser for `nvidia-smi` memory output, define `VramInfo` structure, and expose detailed auditing summary.
2. `crates/vox-populi/src/mens/tensor/memory_budget.rs`: Add Qwen 3 ladder, compute dynamic resident footprint base + offsets, and add `_with_options` planning functions.
3. `crates/vox-populi/src/mens/tensor/preset_schema.rs`: Dynamically bound manual preset configs based on VRAM planning to avoid OOM.
4. `crates/vox-ml-cli/src/commands/mens/populi/train_arm.rs`: Add diagnostic VRAM audit log during command startup.
5. `crates/vox-ml-cli/src/commands/schola/train/gpu.rs`: Auto-enable gradient checkpointing for any model >= 2.9B parameters.

---

## Task List

### Task 1: Implement Dynamic VRAM Auditing in `vram_autodetect.rs`

**Files:**
- Modify: `crates/vox-populi/src/mens/tensor/vram_autodetect.rs`

- [ ] **Step 1: Write the failing tests**

Add these tests to `crates/vox-populi/src/mens/tensor/vram_autodetect.rs` under `mod semcov_wave26_tests`:

```rust
    #[test]
    fn test_parse_nvidia_smi_output() {
        let sample = "16376, 1401, 14645\n";
        let info = parse_nvidia_smi_output(sample).unwrap();
        assert_eq!(info.total_gb, 16376.0 / 1024.0);
        assert_eq!(info.used_gb, 1401.0 / 1024.0);
        assert_eq!(info.free_gb, 14645.0 / 1024.0);
    }

    #[test]
    fn test_parse_nvidia_smi_output_invalid() {
        assert!(parse_nvidia_smi_output("invalid").is_none());
        assert!(parse_nvidia_smi_output("16376, 1401").is_none());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p vox-populi --test-threads=1 test_parse_nvidia_smi_output
```
Expected: FAIL due to missing functions `parse_nvidia_smi_output` and `VramInfo`.

- [ ] **Step 3: Implement VRAM auditing logic**

Add the struct and functions to `crates/vox-populi/src/mens/tensor/vram_autodetect.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VramInfo {
    pub total_gb: f32,
    pub used_gb: f32,
    pub free_gb: f32,
}

fn parse_nvidia_smi_output(stdout: &str) -> Option<VramInfo> {
    let first_line = stdout.lines().next()?.trim();
    let parts: Vec<&str> = first_line.split(',').map(|s| s.trim()).collect();
    if parts.len() < 3 {
        return None;
    }
    let total_mib: f32 = parts[0].parse().ok()?;
    let used_mib: f32 = parts[1].parse().ok()?;
    let free_mib: f32 = parts[2].parse().ok()?;
    Some(VramInfo {
        total_gb: total_mib / 1024.0,
        used_gb: used_mib / 1024.0,
        free_gb: free_mib / 1024.0,
    })
}

/// Query total, used, and free VRAM info from `nvidia-smi`.
pub fn query_nvidia_smi_vram() -> Option<VramInfo> {
    let out = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.total,memory.used,memory.free", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_nvidia_smi_output(&String::from_utf8_lossy(&out.stdout))
}

/// Query available GPU VRAM info.
pub fn get_system_vram_info() -> Option<VramInfo> {
    // Priority 1: env override
    if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxVramOverrideGb).expose()
        && let Ok(gb) = v.parse::<f32>()
        && gb > 0.0
    {
        return Some(VramInfo {
            total_gb: gb,
            used_gb: 0.0,
            free_gb: gb,
        });
    }

    // Priority 2: nvidia-smi query
    if let Some(info) = query_nvidia_smi_vram() {
        return Some(info);
    }

    // Priority 3: hardware SSOT
    let hardware = futures::executor::block_on(crate::mens::hardware::probe());
    if hardware.vram_mb > 0 {
        let gb = hardware.vram_mb as f32 / 1024.0;
        return Some(VramInfo {
            total_gb: gb,
            used_gb: 0.0,
            free_gb: gb,
        });
    }

    None
}
```

Update existing `get_system_vram_gb` and `vram_summary`:

```rust
pub fn get_system_vram_gb() -> Option<f32> {
    get_system_vram_info().map(|i| i.total_gb)
}

pub fn vram_summary(device_is_cuda: bool) -> String {
    let info = get_system_vram_info();
    let preset = auto_preset(device_is_cuda, info.map(|i| i.total_gb));
    match (info, preset) {
        (Some(i), Some(p)) => format!(
            "VRAM: {:.1} GiB total, {:.1} GiB used, {:.1} GiB free → preset '{p}'",
            i.total_gb, i.used_gb, i.free_gb
        ),
        (Some(i), None) => format!(
            "VRAM: {:.1} GiB total, {:.1} GiB used, {:.1} GiB free (no matching preset; specify --preset manually)",
            i.total_gb, i.used_gb, i.free_gb
        ),
        (None, _) => {
            "Could not detect VRAM (set VOX_VRAM_OVERRIDE_GB or pass --preset manually)".to_string()
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p vox-populi --test-threads=1 test_parse_nvidia_smi_output
```
Expected: PASS

- [ ] **Step 5: Commit changes**

Run:
```powershell
git add crates/vox-populi/src/mens/tensor/vram_autodetect.rs
git commit -m "feat: add parsed nvidia-smi VRAM auditing"
```

---

### Task 2: Implement Qwen 3 Ladder and Dynamic Scaling in `memory_budget.rs`

**Files:**
- Modify: `crates/vox-populi/src/mens/tensor/memory_budget.rs`

- [ ] **Step 1: Write the failing tests**

Add these tests to `crates/vox-populi/src/mens/tensor/memory_budget.rs` under `mod tests`:

```rust
    #[test]
    fn test_is_qwen3_detection() {
        assert!(is_qwen3("Qwen/Qwen3-7B"));
        assert!(!is_qwen3("Qwen/Qwen3.5-4B"));
    }

    #[test]
    fn test_get_resident_per_b_scaling() {
        use crate::mens::tensor::finetune_contract::BaseQuantMode;
        // Qwen 2.5/3: base 5.0. NF4 = +0.0. GC = -1.8 -> 3.2
        assert!((get_resident_per_b("Qwen/Qwen2.5-Coder-7B-Instruct", BaseQuantMode::Nf4, true) - 3.2).abs() < 1e-9);
        // Qwen 2.5/3: base 5.0. None = +1.5. No GC = +0.0 -> 6.5
        assert!((get_resident_per_b("Qwen/Qwen2.5-Coder-7B-Instruct", BaseQuantMode::None, false) - 6.5).abs() < 1e-9);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p vox-populi --test-threads=1 test_is_qwen3_detection
```
Expected: FAIL due to missing declarations.

- [ ] **Step 3: Implement Qwen 3 ladder and dynamic scaling**

Add definitions to `crates/vox-populi/src/mens/tensor/memory_budget.rs`:

```rust
use crate::mens::tensor::finetune_contract::BaseQuantMode;

pub const QWEN3_LADDER: &[(f64, &str)] = &[
    (72.0, "Qwen/Qwen3-72B"),
    (32.0, "Qwen/Qwen3-32B"),
    (14.0, "Qwen/Qwen3-14B"),
    (7.0, "Qwen/Qwen3-7B"),
    (3.0, "Qwen/Qwen3-3B"),
    (1.5, "Qwen/Qwen3-1.5B"),
    (0.5, "Qwen/Qwen3-0.5B"),
];

pub fn is_qwen3(model_id: &str) -> bool {
    let l = model_id.to_ascii_lowercase();
    (l.contains("qwen3") || l.contains("qwen-3")) && !is_qwen35(model_id)
}

pub fn get_resident_per_b(
    model_hint: &str,
    quant: BaseQuantMode,
    gradient_checkpointing: bool,
) -> f64 {
    let base = if is_qwen35(model_hint) {
        3.5
    } else {
        5.0 // Qwen 2.5, Qwen 3, default
    };

    let quant_offset = match quant {
        BaseQuantMode::Nf4 => 0.0,
        BaseQuantMode::None => 1.5,
    };

    let gc_offset = if gradient_checkpointing {
        -1.8
    } else {
        0.0
    };

    (base + quant_offset + gc_offset).max(1.5)
}

pub fn plan_qwen35_with_options(
    vram_gib: f64,
    max_params_b: f64,
    quant: BaseQuantMode,
    gradient_checkpointing: bool,
) -> ModelPlan {
    let mut smallest_tried: Option<ModelPlan> = None;
    for &(params, id) in QWEN35_LADDER {
        if params > max_params_b + 1e-9 {
            continue;
        }
        let resident_per_b = get_resident_per_b(id, quant, gradient_checkpointing);
        let p = plan_with_resident(vram_gib, params, resident_per_b);
        let retreated = (params - max_params_b).abs() > 1e-9;
        let retreated_from_b = retreated.then_some(max_params_b);
        let rationale = if retreated {
            format!(
                "requested ≈{max_params_b:.1}B does not fit {vram_gib:.0} GiB; retreated to \
                 {id} — {}",
                p.rationale
            )
        } else {
            format!("{id} — {}", p.rationale)
        };
        let mp = ModelPlan {
            model_id: id.to_string(),
            params_b: params,
            seq_len: p.seq_len,
            batch_size: p.batch_size,
            grad_accum: p.grad_accum,
            retreated_from_b,
            over_budget: p.over_budget,
            rationale,
        };
        if !p.over_budget {
            return mp;
        }
        smallest_tried = Some(mp);
    }
    smallest_tried.unwrap_or_else(|| {
        let (params, id) = *QWEN35_LADDER.last().unwrap();
        let resident_per_b = get_resident_per_b(id, quant, gradient_checkpointing);
        let p = plan_with_resident(vram_gib, params, resident_per_b);
        ModelPlan {
            model_id: id.to_string(),
            params_b: params,
            seq_len: p.seq_len,
            batch_size: p.batch_size,
            grad_accum: p.grad_accum,
            retreated_from_b: Some(max_params_b),
            over_budget: true,
            rationale: format!("no Qwen3.5 variant fits {vram_gib:.0} GiB; {}", p.rationale),
        }
    })
}

pub fn plan_qwen3_with_options(
    vram_gib: f64,
    max_params_b: f64,
    quant: BaseQuantMode,
    gradient_checkpointing: bool,
) -> ModelPlan {
    let mut smallest_tried: Option<ModelPlan> = None;
    for &(params, id) in QWEN3_LADDER {
        if params > max_params_b + 1e-9 {
            continue;
        }
        let resident_per_b = get_resident_per_b(id, quant, gradient_checkpointing);
        let p = plan_with_resident(vram_gib, params, resident_per_b);
        let retreated = (params - max_params_b).abs() > 1e-9;
        let rationale = if retreated {
            format!(
                "requested ≈{max_params_b:.1}B does not fit {vram_gib:.0} GiB; retreated to {id} — {}",
                p.rationale
            )
        } else {
            format!("{id} — {}", p.rationale)
        };
        let mp = ModelPlan {
            model_id: id.to_string(),
            params_b: params,
            seq_len: p.seq_len,
            batch_size: p.batch_size,
            grad_accum: p.grad_accum,
            retreated_from_b: retreated.then_some(max_params_b),
            over_budget: p.over_budget,
            rationale,
        };
        if !p.over_budget {
            return mp;
        }
        smallest_tried = Some(mp);
    }
    smallest_tried.unwrap_or_else(|| {
        let (params, id) = *QWEN3_LADDER.last().unwrap();
        let resident_per_b = get_resident_per_b(id, quant, gradient_checkpointing);
        let p = plan_with_resident(vram_gib, params, resident_per_b);
        ModelPlan {
            model_id: id.to_string(),
            params_b: params,
            seq_len: p.seq_len,
            batch_size: p.batch_size,
            grad_accum: p.grad_accum,
            retreated_from_b: Some(max_params_b),
            over_budget: true,
            rationale: format!(
                "no Qwen3 variant fits {vram_gib:.0} GiB; {}",
                p.rationale
            ),
        }
    })
}

pub fn plan_qwen3(vram_gib: f64, max_params_b: f64) -> ModelPlan {
    plan_qwen3_with_options(vram_gib, max_params_b, BaseQuantMode::Nf4, false)
}

pub fn plan_qwen25coder_with_options(
    vram_gib: f64,
    max_params_b: f64,
    quant: BaseQuantMode,
    gradient_checkpointing: bool,
) -> ModelPlan {
    let mut smallest_tried: Option<ModelPlan> = None;
    for &(params, id) in QWEN25CODER_LADDER {
        if params > max_params_b + 1e-9 {
            continue;
        }
        let resident_per_b = get_resident_per_b(id, quant, gradient_checkpointing);
        let p = plan_with_resident(vram_gib, params, resident_per_b);
        let retreated = (params - max_params_b).abs() > 1e-9;
        let rationale = if retreated {
            format!(
                "requested ≈{max_params_b:.1}B does not fit {vram_gib:.0} GiB; retreated to {id} — {}",
                p.rationale
            )
        } else {
            format!("{id} — {}", p.rationale)
        };
        let mp = ModelPlan {
            model_id: id.to_string(),
            params_b: params,
            seq_len: p.seq_len,
            batch_size: p.batch_size,
            grad_accum: p.grad_accum,
            retreated_from_b: retreated.then_some(max_params_b),
            over_budget: p.over_budget,
            rationale,
        };
        if !p.over_budget {
            return mp;
        }
        smallest_tried = Some(mp);
    }
    smallest_tried.unwrap_or_else(|| {
        let (params, id) = *QWEN25CODER_LADDER.last().unwrap();
        let resident_per_b = get_resident_per_b(id, quant, gradient_checkpointing);
        let p = plan_with_resident(vram_gib, params, resident_per_b);
        ModelPlan {
            model_id: id.to_string(),
            params_b: params,
            seq_len: p.seq_len,
            batch_size: p.batch_size,
            grad_accum: p.grad_accum,
            retreated_from_b: Some(max_params_b),
            over_budget: true,
            rationale: format!(
                "no Qwen2.5-Coder variant fits {vram_gib:.0} GiB; {}",
                p.rationale
            ),
        }
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p vox-populi --test-threads=1 test_is_qwen3_detection
```
Expected: PASS

- [ ] **Step 5: Commit changes**

Run:
```powershell
git add crates/vox-populi/src/mens/tensor/memory_budget.rs
git commit -m "feat: add Qwen 3 ladder and dynamic resident memory scaling"
```

---

### Task 3: Bound Hyperparameter Presets Dynamically in `preset_schema.rs`

**Files:**
- Modify: `crates/vox-populi/src/mens/tensor/preset_schema.rs`

- [ ] **Step 1: Write the failing tests**

Add this test to `preset_schema.rs` under `mod preset_tests`:

```rust
    #[test]
    fn test_preset_bounds_dynamically_to_fit_vram() {
        let dev = DeviceProfile::from_gpu_info("rtx 4080 super", 16384);
        let profile = resolve_effective_profile(Some("prosumer_16g"), dev, None, CliOverrides::default());
        // For a 7B model on 16GB, it should safely scale parameters down.
        assert!(profile.seq_len <= 384);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p vox-populi --test-threads=1 test_preset_bounds_dynamically_to_fit_vram
```
Expected: FAIL if the dynamic bounds logic isn't hooked up yet.

- [ ] **Step 3: Implement dynamic preset bounds**

Modify `resolve_effective_profile` in `crates/vox-populi/src/mens/tensor/preset_schema.rs` to run the budget planner against the resolved profile:

```rust
use crate::mens::tensor::memory_budget::{self, BaseQuantMode};

pub fn resolve_effective_profile(
    preset: Option<&str>,
    device: DeviceProfile,
    sample_count: Option<usize>,
    overrides: CliOverrides,
) -> TrainPresetProfile {
    let model_hint_resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxBaseModel);
    let model_hint = model_hint_resolved.expose();
    let env_p_resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxTrainProfile);
    let env_p = env_p_resolved.expose();
    let name = normalize_preset_name(preset.or(env_p).unwrap_or(DEFAULT_PRESET));

    let mut p = if name == "auto" {
        if let Some(specs) = load_gpu_specs() {
            if let Some((_name, preset_spec)) =
                TrainingPreset::best_for_vram(&specs.presets, device.vram_mb)
            {
                TrainPresetProfile {
                    rank: 16,
                    alpha: 32.0,
                    seq_len: preset_spec.seq_len,
                    batch_size: preset_spec.batch_size,
                    grad_accum: preset_spec.grad_accum,
                    epochs: 3,
                    warmup: 100,
                    lr: preset_spec.lr,
                }
            } else {
                base_for_name("4080_safe")
            }
        } else {
            base_for_name("4080_safe")
        }
    } else {
        base_for_name(name)
    };

    // Dynamically bound preset parameters based on VRAM planning for the target model.
    if device.vram_mb > 0 {
        let vram_gib = device.vram_mb as f64 / 1024.0;
        let model_str = model_hint.as_deref().unwrap_or("Qwen/Qwen2.5-Coder-7B-Instruct");
        let requested_b = memory_budget::params_b_from_model_hint(model_str).unwrap_or(7.0);

        let gc_explicit = std::env::var("VOX_MENS_GRADIENT_CHECKPOINTING")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let gc_auto = requested_b >= 2.9;
        let gradient_checkpointing = gc_explicit || gc_auto;

        let quant = if name == "safe" || name == "vox-gen" || name == "4080" || name == "qwen_4080_16g" {
            BaseQuantMode::Nf4
        } else {
            BaseQuantMode::Nf4
        };

        let budget_plan = if memory_budget::is_qwen25coder(model_str) {
            memory_budget::plan_qwen25coder_with_options(vram_gib, requested_b, quant, gradient_checkpointing)
        } else if memory_budget::is_qwen3(model_str) {
            memory_budget::plan_qwen3_with_options(vram_gib, requested_b, quant, gradient_checkpointing)
        } else {
            memory_budget::plan_qwen35_with_options(vram_gib, requested_b, quant, gradient_checkpointing)
        };

        p.seq_len = p.seq_len.min(budget_plan.seq_len);
        p.batch_size = p.batch_size.min(budget_plan.batch_size);
        p.grad_accum = p.grad_accum.max(budget_plan.grad_accum);
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p vox-populi --test-threads=1 test_preset_bounds_dynamically_to_fit_vram
```
Expected: PASS

- [ ] **Step 5: Commit changes**

Run:
```powershell
git add crates/vox-populi/src/mens/tensor/preset_schema.rs
git commit -m "feat: enforce dynamic preset bounding based on VRAM plan"
```

---

### Task 4: Integrate VRAM Auditing in `train_arm.rs`

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/mens/populi/train_arm.rs`

- [ ] **Step 1: Verify compilation before modifications**

Run:
```powershell
cargo check -p vox-ml-cli
```
Expected: OK

- [ ] **Step 2: Add dynamic VRAM audit reporting and options query**

Modify `train_arm.rs` starting at line 256 to use the audited free VRAM and dynamic planning options:

```rust
    // ── VRAM-aware memory budgeting ──────────────────────────────────────────
    // When seq_len / batch_size / grad_accum were not pinned by the user or a
    // domain profile, size them from the detected VRAM and model size so the run
    // fits without OOM and still maximizes utilization. Only applies on CUDA.
    {
        use owo_colors::OwoColorize;
        let device_is_cuda = vox_populi::mens::normalize_device(&device)
            .map(|d| matches!(d, vox_populi::mens::DeviceKind::Cuda))
            .unwrap_or(false);
        if device_is_cuda {
            use vox_populi::mens::tensor::memory_budget::{self, BaseQuantMode};
            let default_model = vox_populi::mens::default_model_id();
            let model_hint = effective_model.as_deref().unwrap_or(&default_model);
            
            // Resolve the actual model parameter count we are sizing for (pinned or default).
            let target_b = memory_budget::params_b_from_model_hint(model_hint).unwrap_or(7.0);

            // Audit the actual available (free) VRAM instead of total VRAM.
            if let Some(vram_info) = vox_populi::mens::tensor::vram_autodetect::get_system_vram_info() {
                let vram = vram_info.free_gb as f64;
                
                eprintln!(
                    "  {} VRAM Audit: Total: {:.1} GiB | Used: {:.1} GiB | Available (Free): {:.1} GiB",
                    "📊".cyan(),
                    vram_info.total_gb,
                    vram_info.used_gb,
                    vram_info.free_gb
                );

                let gc_explicit = std::env::var("VOX_MENS_GRADIENT_CHECKPOINTING")
                    .ok()
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                let gc_auto = target_b >= 2.9;
                let gradient_checkpointing = gc_explicit || gc_auto;

                let quant = if matches!(backend, PopuliTrainBackendCli::Qlora) {
                    BaseQuantMode::Nf4
                } else {
                    BaseQuantMode::None
                };

                // Coding-focused dense Qwen2.5-Coder ladder.
                let mp = if memory_budget::is_qwen25coder(model_hint) {
                    memory_budget::plan_qwen25coder_with_options(vram, target_b, quant, gradient_checkpointing)
                } else if memory_budget::is_qwen3(model_hint) {
                    memory_budget::plan_qwen3_with_options(vram, target_b, quant, gradient_checkpointing)
                } else {
                    memory_budget::plan_qwen35_with_options(vram, target_b, quant, gradient_checkpointing)
                };

                eprintln!("  {} VRAM budget plan: {}", "⚙".cyan(), mp.rationale);
                if let Some(from_b) = mp.retreated_from_b {
                    if effective_model.is_none() {
                        eprintln!(
                            "  {} Auto-selected {} for {:.0} GiB VRAM (requested ≈{:.1}B would not fit).",
                            "↓".yellow(),
                            mp.model_id,
                            vram,
                            from_b
                        );
                        effective_model = Some(mp.model_id.clone());
                    } else {
                        eprintln!(
                            "  {} {} is pinned but may not fit {:.0} GiB — omit --model to auto-retreat to {}.",
                            "⚠".yellow(),
                            model_hint,
                            vram,
                            mp.model_id
                        );
                    }
                }

                effective_seq_len =
                    resolve_training_sizing(effective_seq_len, None, Some(mp.seq_len), None);
                effective_batch_size = resolve_training_sizing(
                    effective_batch_size,
                    None,
                    Some(mp.batch_size),
                    None,
                );
                effective_grad_accum = resolve_training_sizing(
                    effective_grad_accum,
                    None,
                    Some(mp.grad_accum),
                    None,
                );
            } else {
                eprintln!(
                    "  {} Could not detect VRAM; using preset defaults. Set VOX_VRAM_OVERRIDE_GB \
                     to enable VRAM-aware sizing.",
                    "⚙".cyan()
                );
            }
        }
    }
```

- [ ] **Step 3: Verify build**

Run:
```powershell
cargo check -p vox-ml-cli
```
Expected: OK

- [ ] **Step 4: Commit changes**

Run:
```powershell
git add crates/vox-ml-cli/src/commands/mens/populi/train_arm.rs
git commit -m "feat: integrate detailed VRAM auditing and options in train_arm"
```

---

### Task 5: Auto-enable Gradient Checkpointing for Large Models in `gpu.rs`

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/schola/train/gpu.rs`

- [ ] **Step 1: Verify compilation before modifications**

Run:
```powershell
cargo check -p vox-ml-cli
```
Expected: OK

- [ ] **Step 2: Modify gradient checkpointing resolution**

Change `gpu.rs` line 256-263 to auto-enable gradient checkpointing for all large models (>= 2.9B parameters):

```rust
    let gc_explicit = std::env::var("VOX_MENS_GRADIENT_CHECKPOINTING")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let gc_auto_large = model
        .as_deref()
        .map(|m| {
            let b = vox_populi::mens::tensor::memory_budget::params_b_from_model_hint(m).unwrap_or(4.0);
            b >= 2.9
        })
        .unwrap_or(false);
    let gradient_checkpointing = gc_explicit || gc_auto_large;
```

- [ ] **Step 3: Run full workspace tests and check build**

Run:
```powershell
cargo test -p vox-populi
cargo test -p vox-ml-cli
```
Expected: PASS

- [ ] **Step 4: Format changes**

Run:
```powershell
cargo fmt -p vox-populi
cargo fmt -p vox-ml-cli
```
Expected: OK

- [ ] **Step 5: Commit final changes**

Run:
```powershell
git add crates/vox-ml-cli/src/commands/schola/train/gpu.rs
git commit -m "feat: auto-enable gradient checkpointing for >= 2.9B parameter models"
```
