# Ludus Guardian Wellness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Ludus Guardian Wellness boundaries (time curfews, consecutive work hour constraints) in the Vox configuration, enforce them via intentional friction delays or hard lockout rejections during task submission, and generate wellness break quests.

**Architecture:** 
1. Extends the `VoxConfig` struct flatly in `vox-config` with the curfew/wellness settings.
2. Exposes configuration keys in the global `LLM_CONFIG_KEYS` registry and hooks them into the `vox-gui` user config mutators.
3. Implements 24-hour time range parsing and curfew checks inside the gamification `config_gate.rs`.
4. Extends GUI task submission (`submit_orchestrator_task`) in `control_plane.rs` to validate wellness limits, adding a 10s async delay for "soft" curfew violations or returning a hard error.

**Tech Stack:** Rust (async/tokio), Tauri runtime, SQLite/Turso.

---

## File Structure

The following files will be created or modified:

| Action | Path | Responsibility |
|---|---|---|
| **Modify** | `crates/vox-config/src/config/vox_config.rs` | Adds `guardian_` wellness fields to the main `VoxConfig` struct and default values. |
| **Modify** | `crates/vox-llm-config/src/keys.rs` | Registers configuration keys in the static `LLM_CONFIG_KEYS` slice. |
| **Modify** | `crates/vox-gui/src/commands/user_config.rs` | Maps the new keys to string-serialisation resolvers and mutators in GUI command bindings. |
| **Modify** | `crates/vox-gamify/src/config_gate.rs` | Implements time parsing and curfew checking helper functions. |
| **Create** | `crates/vox-gamify/tests/guardian_wellness_tests.rs` | Unit and integration test suite validating curfew math, 24-hour boundary crossing, and config default values. |
| **Modify** | `crates/vox-gui/src/commands/control_plane.rs` | Extends task submission to run wellness validations, implementing hard curfew rejections and soft intentional friction sleeps. |

---

## Tasks

### Task 1: Extend `VoxConfig` with Guardian Settings

**Files:**
- Modify: [vox_config.rs](file:///c:/Users/Owner/vox/crates/vox-config/src/config/vox_config.rs)

- [ ] **Step 1: Edit Struct Definitions**
  Add the new fields to `VoxConfig` struct and their default values.

  In [vox_config.rs](file:///c:/Users/Owner/vox/crates/vox-config/src/config/vox_config.rs):
  ```rust
  // Modify VoxConfig struct starting at line 13:
  pub struct VoxConfig {
      // Existing fields ...
      pub daily_budget_usd: f64,
      pub per_session_budget_usd: f64,
      pub data_dir: PathBuf,
      pub model_dir: PathBuf,
      pub train_epochs: usize,
      pub train_batch_size: usize,
      pub mcp_binary: Option<PathBuf>,
      pub db_url: Option<String>,
      pub gamify_enabled: bool,
      pub gamify_mode: GamifyMode,
      pub web_run_mode: WebRunMode,
      pub web_tanstack_start: bool,
      pub build_target: BuildTarget,
      pub hitl: HitlPolicy,
      pub llm_max_concurrent_requests: usize,
      pub llm_openrouter_max_concurrent: Option<usize>,
      pub llm_openai_max_concurrent: Option<usize>,
      pub llm_retry_max_attempts: u32,
      // NEW wellness config fields:
      pub guardian_enable_curfew: bool,
      pub guardian_curfew_start: String,
      pub guardian_curfew_end: String,
      pub guardian_max_consecutive_hours: f64,
      pub guardian_lockout_mode: String, // "none" | "soft" | "hard"
  }
  ```

- [ ] **Step 2: Add Defaults to `impl Default for VoxConfig`**
  Set the standard defaults (disabled, curfew start 22:00, curfew end 06:00, max hours 3.0, lockout "soft").

  In [vox_config.rs](file:///c:/Users/Owner/vox/crates/vox-config/src/config/vox_config.rs):
  ```rust
  // Modify impl Default for VoxConfig starting at line 43:
  impl Default for VoxConfig {
      fn default() -> Self {
          Self {
              // Existing fields ...
              model: "anthropic/claude-sonnet-4".to_string(),
              openrouter_key: None,
              openai_key: None,
              gemini_key: None,
              anthropic_key: None,
              daily_budget_usd: 5.0,
              per_session_budget_usd: 1.0,
              data_dir: PathBuf::from("target/dogfood"),
              model_dir: crate::paths::data_dir()
                  .map(|d| d.join("models"))
                  .unwrap_or_else(|| PathBuf::from(crate::paths::REPO_MODELS_DIR)),
              train_epochs: 3,
              train_batch_size: 256,
              mcp_binary: None,
              db_url: None,
              gamify_enabled: true,
              gamify_mode: GamifyMode::default(),
              web_run_mode: WebRunMode::default(),
              web_tanstack_start: false,
              build_target: BuildTarget::default(),
              hitl: HitlPolicy::default(),
              llm_max_concurrent_requests: 8,
              llm_openrouter_max_concurrent: None,
              llm_openai_max_concurrent: None,
              llm_retry_max_attempts: 4,
              // NEW default values:
              guardian_enable_curfew: false,
              guardian_curfew_start: "22:00".to_string(),
              guardian_curfew_end: "06:00".to_string(),
              guardian_max_consecutive_hours: 3.0,
              guardian_lockout_mode: "soft".to_string(),
          }
      }
  }
  ```

- [ ] **Step 3: Run Compiler Check**
  Verify the struct modification compiles.

  Run: `cargo check -p vox-config`
  Expected Output: Successful compilation with zero errors.

- [ ] **Step 4: Commit Changes**
  ```bash
  git add crates/vox-config/src/config/vox_config.rs
  git commit -m "feat(ludus): add guardian configuration parameters to VoxConfig"
  ```

---

### Task 2: Register Config Keys in static registries and CLI resolvers

**Files:**
- Modify: [keys.rs](file:///c:/Users/Owner/vox/crates/vox-llm-config/src/keys.rs)
- Modify: [user_config.rs](file:///c:/Users/Owner/vox/crates/vox-gui/src/commands/user_config.rs)

- [ ] **Step 1: Modify `keys.rs` to register settings**
  Add key definitions at the bottom of the `LLM_CONFIG_KEYS` slice.

  In [keys.rs](file:///c:/Users/Owner/vox/crates/vox-llm-config/src/keys.rs):
  ```rust
  // Insert at the end of LLM_CONFIG_KEYS slice (line 140):
      vc_key!("guardian_enable_curfew", Bool, General, "Guardian enable curfew", "Enable/disable curfew limits"),
      vc_key!("guardian_curfew_start", String, General, "Guardian curfew start time", "Start time for curfew range (e.g. 22:00)"),
      vc_key!("guardian_curfew_end", String, General, "Guardian curfew end time", "End time for curfew range (e.g. 06:00)"),
      vc_key!("guardian_max_consecutive_hours", Float, General, "Guardian max consecutive hours", "Consecutive hours limit"),
      vc_key!("guardian_lockout_mode", String, General, "Guardian curfew lockout mode", "Lockout mode: none | soft | hard"),
  ];
  ```

- [ ] **Step 2: Add values mapping to `voxconfig_value`**
  Modify the getter in `user_config.rs`.

  In [user_config.rs](file:///c:/Users/Owner/vox/crates/vox-gui/src/commands/user_config.rs):
  ```rust
  // Modify voxconfig_value starting at line 44:
  fn voxconfig_value(cfg: &vox_config::VoxConfig, key: &str) -> String {
      match key {
          "model" => cfg.model.clone(),
          "daily_budget_usd" => cfg.daily_budget_usd.to_string(),
          "per_session_budget_usd" => cfg.per_session_budget_usd.to_string(),
          "data_dir" => cfg.data_dir.to_string_lossy().into_owned(),
          "db_url" => cfg.db_url.clone().unwrap_or_default(),
          "train_epochs" => cfg.train_epochs.to_string(),
          "train_batch_size" => cfg.train_batch_size.to_string(),
          // NEW bindings:
          "guardian_enable_curfew" => cfg.guardian_enable_curfew.to_string(),
          "guardian_curfew_start" => cfg.guardian_curfew_start.clone(),
          "guardian_curfew_end" => cfg.guardian_curfew_end.clone(),
          "guardian_max_consecutive_hours" => cfg.guardian_max_consecutive_hours.to_string(),
          "guardian_lockout_mode" => cfg.guardian_lockout_mode.clone(),
          _ => String::new(),
      }
  }
  ```

- [ ] **Step 3: Add mutators mapping to `apply_voxconfig_field`**
  Modify the setter in `user_config.rs`.

  In [user_config.rs](file:///c:/Users/Owner/vox/crates/vox-gui/src/commands/user_config.rs):
  ```rust
  // Modify apply_voxconfig_field starting at line 251:
  fn apply_voxconfig_field(
      cfg: &mut vox_config::VoxConfig,
      key: &str,
      value: &str,
  ) -> Result<(), String> {
      match key {
          "model" => cfg.model = value.to_string(),
          "daily_budget_usd" => {
              cfg.daily_budget_usd = value.parse().map_err(|_| "invalid number".to_string())?;
          }
          "per_session_budget_usd" => {
              cfg.per_session_budget_usd = value.parse().map_err(|_| "invalid number".to_string())?;
          }
          "data_dir" => cfg.data_dir = std::path::PathBuf::from(value),
          "db_url" => cfg.db_url = Some(value.to_string()),
          "train_epochs" => {
              cfg.train_epochs = value.parse().map_err(|_| "invalid integer".to_string())?;
          }
          "train_batch_size" => {
              cfg.train_batch_size = value.parse().map_err(|_| "invalid integer".to_string())?;
          }
          // NEW bindings:
          "guardian_enable_curfew" => {
              cfg.guardian_enable_curfew = value.parse().map_err(|_| "invalid boolean".to_string())?;
          }
          "guardian_curfew_start" => cfg.guardian_curfew_start = value.to_string(),
          "guardian_curfew_end" => cfg.guardian_curfew_end = value.to_string(),
          "guardian_max_consecutive_hours" => {
              cfg.guardian_max_consecutive_hours = value.parse().map_err(|_| "invalid number".to_string())?;
          }
          "guardian_lockout_mode" => {
              let lower = value.to_lowercase();
              if matches!(lower.as_str(), "none" | "soft" | "hard") {
                  cfg.guardian_lockout_mode = lower;
              } else {
                  return Err("invalid lockout mode (must be none, soft, or hard)".to_string());
              }
          }
          _ => return Err(format!("not a VoxConfig field: {key}")),
      }
      Ok(())
  }
  ```

- [ ] **Step 4: Run compiler check**
  Run: `cargo check -p vox-llm-config` and `cargo check -p vox-gui`
  Expected Output: Both targets compile successfully.

- [ ] **Step 5: Commit changes**
  ```bash
  git add crates/vox-llm-config/src/keys.rs crates/vox-gui/src/commands/user_config.rs
  git commit -m "feat(ludus): bind guardian settings keys to global config and GUI registry"
  ```

---

### Task 3: Implement Curfew Math & Wellness Rules inside Gamification

**Files:**
- Modify: [config_gate.rs](file:///c:/Users/Owner/vox/crates/vox-gamify/src/config_gate.rs)
- Create: `crates/vox-gamify/tests/guardian_wellness_tests.rs`

- [ ] **Step 1: Write a Failing TDD Test Suite**
  Create a new test file validating 24h curfew math and boundary crossing (e.g. 22:00 to 06:00).

  In `crates/vox-gamify/tests/guardian_wellness_tests.rs`:
  ```rust
  use vox_gamify::config_gate::{is_time_in_range, check_curfew_active};
  use vox_config::VoxConfig;

  #[test]
  fn test_is_time_in_range_normal() {
      // Curfew: 09:00 -> 17:00 (no midnight cross)
      assert!(is_time_in_range("12:00", "09:00", "17:00"));
      assert!(is_time_in_range("09:00", "09:00", "17:00")); // inclusive start
      assert!(!is_time_in_range("08:59", "09:00", "17:00"));
      assert!(!is_time_in_range("17:00", "09:00", "17:00")); // exclusive end
      assert!(!is_time_in_range("22:00", "09:00", "17:00"));
  }

  #[test]
  fn test_is_time_in_range_midnight_crossing() {
      // Curfew: 22:00 -> 06:00 (crosses midnight)
      assert!(is_time_in_range("23:30", "22:00", "06:00"));
      assert!(is_time_in_range("01:15", "22:00", "06:00"));
      assert!(is_time_in_range("22:00", "22:00", "06:00"));
      assert!(!is_time_in_range("21:59", "22:00", "06:00"));
      assert!(!is_time_in_range("06:00", "22:00", "06:00"));
      assert!(!is_time_in_range("12:00", "22:00", "06:00"));
  }

  #[test]
  fn test_check_curfew_active_disabled() {
      let mut config = VoxConfig::default();
      config.guardian_enable_curfew = false;
      config.guardian_curfew_start = "22:00".to_string();
      config.guardian_curfew_end = "06:00".to_string();
      
      // Even if time is in range, curfew is disabled
      assert!(!check_curfew_active(&config, "23:00"));
  }
  ```

- [ ] **Step 2: Run test and verify it fails**
  Run: `cargo test -p vox-gamify --test guardian_wellness_tests`
  Expected Output: Test failure because functions `is_time_in_range` and `check_curfew_active` do not exist.

- [ ] **Step 3: Implement Curfew Math in `config_gate.rs`**
  Add helper functions to parse times and run 24h clock comparisons.

  In [config_gate.rs](file:///c:/Users/Owner/vox/crates/vox-gamify/src/config_gate.rs):
  ```rust
  // Add at the bottom of the file:
  
  /// Helper to check if a 24-hour time string "HH:MM" falls inside a curfew range.
  /// Supports ranges crossing midnight (e.g. 22:00 to 06:00).
  pub fn is_time_in_range(current: &str, start: &str, end: &str) -> bool {
      let parse_time = |t: &str| -> Option<(u32, u32)> {
          let parts: Vec<&str> = t.split(':').collect();
          if parts.len() != 2 { return None; }
          let h = parts[0].trim().parse().ok()?;
          let m = parts[1].trim().parse().ok()?;
          if h < 24 && m < 60 { Some((h, m)) } else { None }
      };

      let (c_h, c_m) = match parse_time(current) { Some(t) => t, None => return false };
      let (s_h, s_m) = match parse_time(start) { Some(t) => t, None => return false };
      let (e_h, e_m) = match parse_time(end) { Some(t) => t, None => return false };

      let c_val = c_h * 60 + c_m;
      let s_val = s_h * 60 + s_m;
      let e_val = e_h * 60 + e_m;

      if s_val <= e_val {
          c_val >= s_val && c_val < e_val
      } else {
          c_val >= s_val || c_val < e_val
      }
  }

  /// Evaluates active curfew status against config settings and a test time.
  pub fn check_curfew_active(config: &vox_config::VoxConfig, current_time: &str) -> bool {
      if !config.guardian_enable_curfew {
          return false;
      }
      is_time_in_range(current_time, &config.guardian_curfew_start, &config.guardian_curfew_end)
  }
  ```

- [ ] **Step 4: Run test and verify it passes**
  Run: `cargo test -p vox-gamify --test guardian_wellness_tests`
  Expected Output: ALL tests pass.

- [ ] **Step 5: Commit changes**
  ```bash
  git add crates/vox-gamify/src/config_gate.rs crates/vox-gamify/tests/guardian_wellness_tests.rs
  git commit -m "test(ludus): add curfew check math tests and implement config_gate wellness helpers"
  ```

---

### Task 4: Enforce Curfews on Task Submission with Intentional Friction

**Files:**
- Modify: [control_plane.rs](file:///c:/Users/Owner/vox/crates/vox-gui/src/commands/control_plane.rs)

- [ ] **Step 1: Add a failing test for submission wellness logic**
  Add unit tests at the bottom of `control_plane.rs` using a mockup validation runner.

  In [control_plane.rs](file:///c:/Users/Owner/vox/crates/vox-gui/src/commands/control_plane.rs):
  ```rust
  // Append to the bottom of control_plane.rs:

  #[cfg(test)]
  mod wellness_tests {
      use super::*;
      use vox_config::VoxConfig;

      #[tokio::test]
      async fn test_validate_submission_wellness_disabled() {
          let mut cfg = VoxConfig::default();
          cfg.guardian_enable_curfew = false;
          // Even if time is in curfew, should return Ok immediately
          let res = validate_submission_wellness(&cfg, "23:00").await;
          assert!(res.is_ok());
      }

      #[tokio::test]
      async fn test_validate_submission_wellness_hard_lockout() {
          let mut cfg = VoxConfig::default();
          cfg.guardian_enable_curfew = true;
          cfg.guardian_curfew_start = "22:00".to_string();
          cfg.guardian_curfew_end = "06:00".to_string();
          cfg.guardian_lockout_mode = "hard".to_string();

          let res = validate_submission_wellness(&cfg, "23:00").await;
          assert!(res.is_err());
          assert_eq!(res.unwrap_err(), "Guardian Curfew Active. Go rest!");
      }

      #[tokio::test]
      async fn test_validate_submission_wellness_soft_friction() {
          let mut cfg = VoxConfig::default();
          cfg.guardian_enable_curfew = true;
          cfg.guardian_curfew_start = "22:00".to_string();
          cfg.guardian_curfew_end = "06:00".to_string();
          cfg.guardian_lockout_mode = "soft".to_string();

          // Time it to verify sleep happened
          let start = std::time::Instant::now();
          let res = validate_submission_wellness(&cfg, "23:00").await;
          assert!(res.is_ok());
          // Soft sleep is 1s in tests or 10s in production; check that sleep was executed (at least 500ms)
          assert!(start.elapsed() >= std::time::Duration::from_millis(500));
      }
  }
  ```

- [ ] **Step 2: Run test and verify it fails**
  Run: `cargo test -p vox-gui --lib commands::control_plane`
  Expected Output: Test compilation fails because `validate_submission_wellness` is not defined.

- [ ] **Step 3: Implement `validate_submission_wellness`**
  Add the validation helper and sleep timer inside `control_plane.rs`.

  In [control_plane.rs](file:///c:/Users/Owner/vox/crates/vox-gui/src/commands/control_plane.rs):
  ```rust
  // Insert starting above the tests module:

  /// Validates task submission against curfew settings.
  /// If curfew is active:
  /// - "hard": Returns a curfew lockout error message.
  /// - "soft": Sleeps for 10 seconds (or 1 second in test mode) to introduce intentional friction.
  pub async fn validate_submission_wellness(
      cfg: &vox_config::VoxConfig,
      current_time: &str,
  ) -> Result<(), String> {
      if !cfg.guardian_enable_curfew {
          return Ok(());
      }
      
      if vox_gamify::config_gate::is_time_in_range(current_time, &cfg.guardian_curfew_start, &cfg.guardian_curfew_end) {
          match cfg.guardian_lockout_mode.as_str() {
              "hard" => {
                  return Err("Guardian Curfew Active. Go rest!".to_string());
              }
              "soft" => {
                  #[cfg(test)]
                  let delay = std::time::Duration::from_secs(1);
                  #[cfg(not(test))]
                  let delay = std::time::Duration::from_secs(10);
                  
                  tokio::time::sleep(delay).await;
              }
              _ => {}
          }
      }
      Ok(())
  }
  ```

- [ ] **Step 4: Integrate wellness check in `submit_orchestrator_task`**
  Call the wellness check at the very beginning of the Tauri command handler.

  In [control_plane.rs](file:///c:/Users/Owner/vox/crates/vox-gui/src/commands/control_plane.rs):
  ```rust
  // Modify submit_orchestrator_task starting at line 45:
  #[tauri::command]
  pub async fn submit_orchestrator_task(
      app_handle: tauri::AppHandle,
      input: SubmitTaskInput,
  ) -> Result<ControlPlaneResult, String> {
      // NEW: Load config and run wellness check:
      let config = vox_config::VoxConfig::load();
      let now = chrono::Local::now().format("%H:%M").to_string();
      validate_submission_wellness(&config, &now).await?;

      // Existing task submission code continues:
      let file_manifest: Vec<FileAffinity> = input.files.iter().map(FileAffinity::write).collect();
      // ...
  ```

- [ ] **Step 5: Run tests and verify they pass**
  Run: `cargo test -p vox-gui --lib commands::control_plane`
  Expected Output: ALL tests pass.

- [ ] **Step 6: Commit changes**
  ```bash
  git add crates/vox-gui/src/commands/control_plane.rs
  git commit -m "feat(ludus): enforce curfew delays/lockout in submit_orchestrator_task Tauri command"
  ```

---

## Verification Plan

### Automated Tests
*   `cargo test -p vox-config`
*   `cargo test -p vox-gamify --test guardian_wellness_tests`
*   `cargo test -p vox-gui --lib commands::control_plane`

### Manual Verification
1. Launch the Vox GUI in dev server mode (`pnpm dev` in `crates/vox-gui/ui`).
2. Navigate to settings, go to the "Gamification" tab.
3. Turn on curfew limits (e.g. set Curfew Start to current hour, Lockout Mode to "soft").
4. Go to the Chat interface, type a prompt and press submit.
5. Verify that the task does not immediately submit, but experiences a visible 10-second delay (intentional friction) before enqueuing.
6. Change the Lockout Mode to "hard" in settings, try submitting another prompt.
7. Verify that the submission fails immediately and displays a clear red error banner: *"Guardian Curfew Active. Go rest!"*
