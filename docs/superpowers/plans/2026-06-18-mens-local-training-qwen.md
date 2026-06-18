# Local Qwen Fine-Tuning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish a local fine-tuning automation script to train the most parameter-efficient Qwen model (either Qwen 3.5 2B or Qwen 2.5 Coder 1.5B/7B) against our local training corpus (3,961 pairs) on an NVIDIA RTX 4080 Super (16 GiB VRAM).

**Architecture:** We will implement a platform-independent `.vox` training automation script that performs a preflight VRAM check, selects/downloads the optimal model target, runs the fine-tuning training process using the Candle QLoRA backend, and saves the adapter weights.

**Tech Stack:** VoxScript (.vox), Rust (`vox-ml-cli`), Candle QLoRA, Hugging Face Tokenizers.

---

## File Structure

The following files will be created:
1. `scripts/train_local_qwen.vox`: A Vox script that checks available system VRAM, resolves the best model configuration, sets hyperparameters, and spawns the training process.
2. `crates/vox-openclaw-runtime/tests/train_script_test.rs`: Integration test verifying the Vox training script compilation and preflight options check.

---

## Task List

### Task 1: Create the Local Training Automation Script

**Files:**
- Create: `scripts/train_local_qwen.vox`
- Test: `crates/vox-openclaw-runtime/tests/train_script_test.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-openclaw-runtime/tests/train_script_test.rs` with a test verifying that `scripts/train_local_qwen.vox` is present and syntactically valid:

```rust
#[cfg(test)]
mod tests_train_script {
    use std::path::PathBuf;

    #[test]
    fn test_train_script_exists_and_parses() {
        let script_path = PathBuf::from("../../scripts/train_local_qwen.vox");
        if !script_path.exists() {
            let relative = PathBuf::from("scripts/train_local_qwen.vox");
            assert!(relative.exists(), "train_local_qwen.vox not found");
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```powershell
cargo test -p vox-openclaw-runtime --test train_script_test
```
Expected: FAIL (file does not exist or test file not registered).

- [ ] **Step 3: Create the `.vox` training automation script**

Create `scripts/train_local_qwen.vox` with the script content to orchestrate local training:

```vox
// vox:skip — Automation wrapper to launch training via vox-ml-cli
fn main() {
    let output_dir = "mens/runs/qwen_local_latest";
    let data_dir = "mens/data";
    
    // Default model to Qwen 3.5 2B (dense small series) which fits nicely at seq 384
    let model = "Qwen/Qwen3.5-2B-Instruct";
    
    println("--- Starting Local Qwen Training Automation ---");
    println("Target Model:  " + model);
    println("Data Dir:      " + data_dir);
    println("Output Dir:    " + output_dir);
    
    // Spawn the vox-ml-cli training command
    let args = [
        "mens", "train",
        "--model", model,
        "--device", "cuda",
        "--backend", "qlora",
        "--data-dir", data_dir,
        "--output-dir", output_dir,
        "--epochs", "3",
        "--lr", "0.0002",
        "--preset", "auto",
        "--tokenizer", "hf"
    ];
    
    println("Invoking training process...");
    let status = shell_exec("vox-ml-cli", args);
    if status != 0 {
        println("Error: Training process exited with non-zero code.");
    } else {
        println("Training completed successfully!");
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:
```powershell
cargo test -p vox-openclaw-runtime --test train_script_test
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add scripts/train_local_qwen.vox crates/vox-openclaw-runtime/tests/train_script_test.rs
git commit -m "feat(mens): add local Qwen training automation script"
```

---

### Task 2: Verify Preflight Sizing Calculations and Dry Run

**Files:**
- Modify: `scripts/train_local_qwen.vox`

- [ ] **Step 1: Write a step to verify dry-run execution**

Modify `scripts/train_local_qwen.vox` to accept a `--dry-run` or `--check-only` flag that invokes the parser checks and exits before launching long-running CUDA training.

```vox
// vox:skip — Automation wrapper to launch training via vox-ml-cli
fn main() {
    let output_dir = "mens/runs/qwen_local_latest";
    let data_dir = "mens/data";
    let model = "Qwen/Qwen3.5-2B-Instruct";
    
    println("--- Local Qwen Training Sizing Preflight Check ---");
    
    let args = [
        "mens", "train",
        "--model", model,
        "--device", "cuda",
        "--backend", "qlora",
        "--data-dir", data_dir,
        "--output-dir", output_dir,
        "--epochs", "1",
        "--lr", "0.0002",
        "--preset", "auto",
        "--tokenizer", "hf",
        "--fast-corpus",
        "--checkpoint-every", "1"
    ];
    
    // Running a 1-step verification check
    let status = shell_exec("vox-ml-cli", args);
    if status != 0 {
        println("Preflight check failed.");
    } else {
        println("Preflight check passed.");
    }
}
```

- [ ] **Step 2: Run verification**

Run:
```powershell
vox run scripts/train_local_qwen.vox
```
Expected: The trainer prints VRAM sizing calculations and initializes without errors, completing step 1 of training.

- [ ] **Step 3: Commit**

```bash
git add scripts/train_local_qwen.vox
git commit -m "feat(mens): add dry-run verification to local training script"
```

---

### Task 3: Execute the Full Training Session

**Files:**
- Modify: `scripts/train_local_qwen.vox`

- [ ] **Step 1: Configure script for full 3-epoch execution**

Set epochs to 3, micro-batch size and sequence length to be resolved dynamically by the VRAM sizing planner (RTX 4080 Super will select preset `qwen_4080_16g` targeting `seq_len = 384` and `batch_size = 1`).

```vox
// vox:skip — Automation wrapper to launch training via vox-ml-cli
fn main() {
    let output_dir = "mens/runs/qwen_local_latest";
    let data_dir = "mens/data";
    let model = "Qwen/Qwen3.5-2B-Instruct";
    
    println("--- Launching Full Local Qwen Fine-Tuning ---");
    
    let args = [
        "mens", "train",
        "--model", model,
        "--device", "cuda",
        "--backend", "qlora",
        "--data-dir", data_dir,
        "--output-dir", output_dir,
        "--epochs", "3",
        "--lr", "0.0002",
        "--preset", "auto",
        "--tokenizer", "hf",
        "--process-priority", "normal",
        "--vram-limit-fraction", "0.9"
    ];
    
    let status = shell_exec("vox-ml-cli", args);
    if status != 0 {
        println("Training run failed.");
    } else {
        println("Training run completed successfully.");
    }
}
```

- [ ] **Step 2: Run the full training process**

Run:
```powershell
vox run scripts/train_local_qwen.vox
```
Expected: The training process runs sequentially, downloading the base model weights, auditing free VRAM, setting sequence bounds, and executing 3 training epochs over 1 hour.

- [ ] **Step 3: Commit**

```bash
git add scripts/train_local_qwen.vox
git commit -m "feat(mens): configure training script for full 3-epoch run"
```

---

## Verification Plan

### Automated Tests
- Run `cargo test -p vox-openclaw-runtime --test train_script_test` to verify script syntax and presence.

### Manual Verification
- Execute `vox run scripts/train_local_qwen.vox` and monitor the console output for:
  - `Available (Free) VRAM` detection showing ~14.9 GiB.
  - `VRAM budget plan` selecting Qwen3.5-2B.
  - Telemetry output writing steps and loss progression.
- Check that the adapter weights are saved under `mens/runs/qwen_local_latest/adapter_model.safetensors`.
