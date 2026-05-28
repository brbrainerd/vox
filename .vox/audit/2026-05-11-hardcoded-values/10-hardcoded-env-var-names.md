# 10 — hardcoded env var names

**Severity**: warning  
**Itemized**: 87

### hv-0685 — `crates/vox-cli-core/src/diagnostics.rs:42`

**Substring**

```text
NO_COLOR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "NO_COLOR" "crates/vox-cli-core/src/diagnostics.rs"`

**Confidence**: high

---

### hv-0686 — `crates/vox-cli-core/src/diagnostics.rs:56`

**Substring**

```text
NO_COLOR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "NO_COLOR" "crates/vox-cli-core/src/diagnostics.rs"`

**Confidence**: high

---

### hv-0687 — `crates/vox-cli/build.rs:9`

**Substring**

```text
CARGO_CFG_TARGET_OS
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_CFG_TARGET_OS" "crates/vox-cli/build.rs"`

**Confidence**: high

---

### hv-0688 — `crates/vox-cli/build.rs:11`

**Substring**

```text
CARGO_CFG_TARGET_ENV
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_CFG_TARGET_ENV" "crates/vox-cli/build.rs"`

**Confidence**: high

---

### hv-0689 — `crates/vox-cli/src/artifact_policy.rs:72`

**Substring**

```text
USERPROFILE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERPROFILE" "crates/vox-cli/src/artifact_policy.rs"`

**Confidence**: high

---

### hv-0690 — `crates/vox-cli/src/artifact_policy.rs:73`

**Substring**

```text
HOME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOME" "crates/vox-cli/src/artifact_policy.rs"`

**Confidence**: high

---

### hv-0691 — `crates/vox-cli/src/build_service.rs:74`

**Substring**

```text
CARGO
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO" "crates/vox-cli/src/build_service.rs"`

**Confidence**: high

---

### hv-0692 — `crates/vox-cli/src/commands/ci/compile_matrix.rs:57`

**Substring**

```text
CARGO
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO" "crates/vox-cli/src/commands/ci/compile_matrix.rs"`

**Confidence**: high

---

### hv-0693 — `crates/vox-cli/src/commands/ci/completion_quality.rs:642`

**Substring**

```text
GITHUB_HEAD_REF
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "GITHUB_HEAD_REF" "crates/vox-cli/src/commands/ci/completion_quality.rs"`

**Confidence**: high

---

### hv-0694 — `crates/vox-cli/src/commands/ci/completion_quality.rs:643`

**Substring**

```text
GITHUB_REF_NAME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "GITHUB_REF_NAME" "crates/vox-cli/src/commands/ci/completion_quality.rs"`

**Confidence**: high

---

### hv-0695 — `crates/vox-cli/src/commands/ci/completion_quality.rs:645`

**Substring**

```text
GITHUB_SHA
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "GITHUB_SHA" "crates/vox-cli/src/commands/ci/completion_quality.rs"`

**Confidence**: high

---

### hv-0696 — `crates/vox-cli/src/commands/ci/dev_loop_audit.rs:35`

**Substring**

```text
CARGO_TARGET_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_TARGET_DIR" "crates/vox-cli/src/commands/ci/dev_loop_audit.rs"`

**Confidence**: high

---

### hv-0697 — `crates/vox-cli/src/commands/ci/gui_smoke.rs:41`

**Substring**

```text
VOX_GUI_PNPM_BUILD
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "VOX_GUI_PNPM_BUILD" "crates/vox-cli/src/commands/ci/gui_smoke.rs"`

**Confidence**: high

---

### hv-0698 — `crates/vox-cli/src/commands/ci/line_endings.rs:158`

**Substring**

```text
GITHUB_BASE_SHA
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "GITHUB_BASE_SHA" "crates/vox-cli/src/commands/ci/line_endings.rs"`

**Confidence**: high

---

### hv-0699 — `crates/vox-cli/src/commands/ci/line_endings.rs:161`

**Substring**

```text
GITHUB_SHA
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "GITHUB_SHA" "crates/vox-cli/src/commands/ci/line_endings.rs"`

**Confidence**: high

---

### hv-0700 — `crates/vox-cli/src/commands/ci/mod.rs:92`

**Substring**

```text
USERPROFILE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERPROFILE" "crates/vox-cli/src/commands/ci/mod.rs"`

**Confidence**: high

---

### hv-0701 — `crates/vox-cli/src/commands/ci/mod.rs:112`

**Substring**

```text
CUDA_PATH
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CUDA_PATH" "crates/vox-cli/src/commands/ci/mod.rs"`

**Confidence**: high

---

### hv-0702 — `crates/vox-cli/src/commands/ci/plugin_abi_parity.rs:65`

**Substring**

```text
CARGO_TARGET_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_TARGET_DIR" "crates/vox-cli/src/commands/ci/plugin_abi_parity.rs"`

**Confidence**: high

---

### hv-0703 — `crates/vox-cli/src/commands/ci/run_body_helpers/cuda.rs:7`

**Substring**

```text
SKIP_CUDA_FEATURE_CHECK
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "SKIP_CUDA_FEATURE_CHECK" "crates/vox-cli/src/commands/ci/run_body_helpers/cuda.rs"`

**Confidence**: high

---

### hv-0704 — `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs:1260`

**Substring**

```text
{managed}
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "{managed}" "crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs"`

**Confidence**: high

---

### hv-0705 — `crates/vox-cli/src/commands/ci/speech_runtime_suite.rs:516`

**Substring**

```text
LOCALAPPDATA
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "LOCALAPPDATA" "crates/vox-cli/src/commands/ci/speech_runtime_suite.rs"`

**Confidence**: high

---

### hv-0706 — `crates/vox-cli/src/commands/clean.rs:48`

**Substring**

```text
HOME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOME" "crates/vox-cli/src/commands/clean.rs"`

**Confidence**: high

---

### hv-0707 — `crates/vox-cli/src/commands/clean.rs:49`

**Substring**

```text
USERPROFILE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERPROFILE" "crates/vox-cli/src/commands/clean.rs"`

**Confidence**: high

---

### hv-0708 — `crates/vox-cli/src/commands/debug.rs:120`

**Substring**

```text
PATH
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "PATH" "crates/vox-cli/src/commands/debug.rs"`

**Confidence**: high

---

### hv-0709 — `crates/vox-cli/src/commands/deploy.rs:262`

**Substring**

```text
USER
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USER" "crates/vox-cli/src/commands/deploy.rs"`

**Confidence**: high

---

### hv-0710 — `crates/vox-cli/src/commands/deploy.rs:263`

**Substring**

```text
USERNAME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERNAME" "crates/vox-cli/src/commands/deploy.rs"`

**Confidence**: high

---

### hv-0711 — `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/compile_target.rs:39`

**Substring**

```text
ANDROID_HOME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "ANDROID_HOME" "crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/compile_target.rs"`

**Confidence**: high

---

### hv-0712 — `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/compile_target.rs:40`

**Substring**

```text
ANDROID_SDK_ROOT
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "ANDROID_SDK_ROOT" "crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/compile_target.rs"`

**Confidence**: high

---

### hv-0713 — `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/compile_target.rs:72`

**Substring**

```text
CARGO
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO" "crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/compile_target.rs"`

**Confidence**: high

---

### hv-0714 — `crates/vox-cli/src/commands/publish.rs:72`

**Substring**

```text
HOME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOME" "crates/vox-cli/src/commands/publish.rs"`

**Confidence**: high

---

### hv-0715 — `crates/vox-cli/src/commands/publish.rs:73`

**Substring**

```text
USERPROFILE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERPROFILE" "crates/vox-cli/src/commands/publish.rs"`

**Confidence**: high

---

### hv-0716 — `crates/vox-cli/src/commands/repo_upgrade.rs:277`

**Substring**

```text
CARGO
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO" "crates/vox-cli/src/commands/repo_upgrade.rs"`

**Confidence**: high

---

### hv-0717 — `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/rules.rs:210`

**Substring**

```text
CARGO
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO" "crates/vox-cli/src/commands/review/coderabbit/semantic_planner/rules.rs"`

**Confidence**: high

---

### hv-0718 — `crates/vox-cli/src/commands/runtime/run/backend/native.rs:125`

**Substring**

```text
CARGO
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO" "crates/vox-cli/src/commands/runtime/run/backend/native.rs"`

**Confidence**: high

---

### hv-0719 — `crates/vox-cli/src/commands/share.rs:109`

**Substring**

```text
VOX_SHARE_CONNECT_TIMEOUT_SECS
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "VOX_SHARE_CONNECT_TIMEOUT_SECS" "crates/vox-cli/src/commands/share.rs"`

**Confidence**: high

---

### hv-0720 — `crates/vox-cli/src/commands/toolchain_upgrade.rs:571`

**Substring**

```text
CARGO_HOME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_HOME" "crates/vox-cli/src/commands/toolchain_upgrade.rs"`

**Confidence**: high

---

### hv-0721 — `crates/vox-cli/src/commands/toolchain_upgrade.rs:575`

**Substring**

```text
USERPROFILE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERPROFILE" "crates/vox-cli/src/commands/toolchain_upgrade.rs"`

**Confidence**: high

---

### hv-0722 — `crates/vox-cli/src/commands/toolchain_upgrade.rs:579`

**Substring**

```text
HOME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOME" "crates/vox-cli/src/commands/toolchain_upgrade.rs"`

**Confidence**: high

---

### hv-0723 — `crates/vox-cli/src/diagnostics.rs:114`

**Substring**

```text
RUST_BACKTRACE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "RUST_BACKTRACE" "crates/vox-cli/src/diagnostics.rs"`

**Confidence**: high

---

### hv-0724 — `crates/vox-cli/src/lib.rs:655`

**Substring**

```text
RUST_LOG
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "RUST_LOG" "crates/vox-cli/src/lib.rs"`

**Confidence**: high

---

### hv-0725 — `crates/vox-cli/src/lock_telemetry.rs:15`

**Substring**

```text
HOME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOME" "crates/vox-cli/src/lock_telemetry.rs"`

**Confidence**: high

---

### hv-0726 — `crates/vox-cli/src/lock_telemetry.rs:16`

**Substring**

```text
USERPROFILE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERPROFILE" "crates/vox-cli/src/lock_telemetry.rs"`

**Confidence**: high

---

### hv-0727 — `crates/vox-code-audit/src/detectors/env_secret_shape.rs:199`

**Substring**

```text
OPENAI_API_KEY
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "OPENAI_API_KEY" "crates/vox-code-audit/src/detectors/env_secret_shape.rs"`

**Confidence**: high

---

### hv-0728 — `crates/vox-code-audit/src/detectors/env_secret_shape.rs:223`

**Substring**

```text
DB_PASSWORD
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "DB_PASSWORD" "crates/vox-code-audit/src/detectors/env_secret_shape.rs"`

**Confidence**: high

---

### hv-0729 — `crates/vox-code-audit/src/detectors/env_secret_shape.rs:232`

**Substring**

```text
EXAMPLE_SECRET_KEY
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "EXAMPLE_SECRET_KEY" "crates/vox-code-audit/src/detectors/env_secret_shape.rs"`

**Confidence**: high

---

### hv-0730 — `crates/vox-code-audit/src/detectors/env_secret_shape.rs:241`

**Substring**

```text
DATABASE_HOST
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "DATABASE_HOST" "crates/vox-code-audit/src/detectors/env_secret_shape.rs"`

**Confidence**: high

---

### hv-0731 — `crates/vox-code-audit/src/detectors/env_secret_shape.rs:250`

**Substring**

```text
API_KEY
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "API_KEY" "crates/vox-code-audit/src/detectors/env_secret_shape.rs"`

**Confidence**: high

---

### hv-0732 — `crates/vox-code-audit/src/detectors/env_secret_shape.rs:259`

**Substring**

```text
FAKE_API_KEY
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "FAKE_API_KEY" "crates/vox-code-audit/src/detectors/env_secret_shape.rs"`

**Confidence**: high

---

### hv-0733 — `crates/vox-code-audit/src/detectors/secrets.rs:238`

**Substring**

```text
STUB_API_KEY
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "STUB_API_KEY" "crates/vox-code-audit/src/detectors/secrets.rs"`

**Confidence**: high

---

### hv-0734 — `crates/vox-compiler/build.rs:60`

**Substring**

```text
CARGO_MANIFEST_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_MANIFEST_DIR" "crates/vox-compiler/build.rs"`

**Confidence**: high

---

### hv-0735 — `crates/vox-compiler/build.rs:156`

**Substring**

```text
OUT_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "OUT_DIR" "crates/vox-compiler/build.rs"`

**Confidence**: high

---

### hv-0736 — `crates/vox-config/src/inference.rs:72`

**Substring**

```text
POPULI_URL
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "POPULI_URL" "crates/vox-config/src/inference.rs"`

**Confidence**: high

---

### hv-0737 — `crates/vox-config/src/inference.rs:73`

**Substring**

```text
OLLAMA_URL
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "OLLAMA_URL" "crates/vox-config/src/inference.rs"`

**Confidence**: high

---

### hv-0738 — `crates/vox-config/src/paths.rs:59`

**Substring**

```text
USERNAME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERNAME" "crates/vox-config/src/paths.rs"`

**Confidence**: high

---

### hv-0739 — `crates/vox-config/src/paths.rs:65`

**Substring**

```text
USER
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USER" "crates/vox-config/src/paths.rs"`

**Confidence**: high

---

### hv-0740 — `crates/vox-config/src/paths.rs:75`

**Substring**

```text
APPDATA
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "APPDATA" "crates/vox-config/src/paths.rs"`

**Confidence**: high

---

### hv-0741 — `crates/vox-config/src/paths.rs:95`

**Substring**

```text
USERPROFILE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERPROFILE" "crates/vox-config/src/paths.rs"`

**Confidence**: high

---

### hv-0742 — `crates/vox-config/src/paths.rs:100`

**Substring**

```text
HOMEDRIVE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOMEDRIVE" "crates/vox-config/src/paths.rs"`

**Confidence**: high

---

### hv-0743 — `crates/vox-config/src/paths.rs:110`

**Substring**

```text
HOME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOME" "crates/vox-config/src/paths.rs"`

**Confidence**: high

---

### hv-0744 — `crates/vox-corpus/build.rs:190`

**Substring**

```text
CARGO_MANIFEST_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_MANIFEST_DIR" "crates/vox-corpus/build.rs"`

**Confidence**: high

---

### hv-0745 — `crates/vox-corpus/build.rs:191`

**Substring**

```text
OUT_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "OUT_DIR" "crates/vox-corpus/build.rs"`

**Confidence**: high

---

### hv-0746 — `crates/vox-http-client/src/lib.rs:12`

**Substring**

```text
CARGO_PKG_VERSION
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_PKG_VERSION" "crates/vox-http-client/src/lib.rs"`

**Confidence**: high

---

### hv-0747 — `crates/vox-mcp-registry/build.rs:49`

**Substring**

```text
CARGO_MANIFEST_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_MANIFEST_DIR" "crates/vox-mcp-registry/build.rs"`

**Confidence**: high

---

### hv-0748 — `crates/vox-mcp-registry/build.rs:90`

**Substring**

```text
OUT_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "OUT_DIR" "crates/vox-mcp-registry/build.rs"`

**Confidence**: high

---

### hv-0749 — `crates/vox-ml-cli/src/commands/corpus/generate.rs:513`

**Substring**

```text
USERPROFILE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERPROFILE" "crates/vox-ml-cli/src/commands/corpus/generate.rs"`

**Confidence**: high

---

### hv-0750 — `crates/vox-ml-cli/src/commands/corpus/generate.rs:514`

**Substring**

```text
HOME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOME" "crates/vox-ml-cli/src/commands/corpus/generate.rs"`

**Confidence**: high

---

### hv-0751 — `crates/vox-ml-cli/src/commands/oratio_cmd.rs:181`

**Substring**

```text
ACTIVE_FILE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "ACTIVE_FILE" "crates/vox-ml-cli/src/commands/oratio_cmd.rs"`

**Confidence**: high

---

### hv-0752 — `crates/vox-ml-cli/src/commands/schola/train/run_train.rs:202`

**Substring**

```text
RUST_LOG
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "RUST_LOG" "crates/vox-ml-cli/src/commands/schola/train/run_train.rs"`

**Confidence**: high

---

### hv-0753 — `crates/vox-orchestrator-mcp/build.rs:18`

**Substring**

```text
CARGO_MANIFEST_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_MANIFEST_DIR" "crates/vox-orchestrator-mcp/build.rs"`

**Confidence**: high

---

### hv-0754 — `crates/vox-orchestrator-mcp/build.rs:29`

**Substring**

```text
OUT_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "OUT_DIR" "crates/vox-orchestrator-mcp/build.rs"`

**Confidence**: high

---

### hv-0755 — `crates/vox-orchestrator-types/build.rs:26`

**Substring**

```text
CARGO_MANIFEST_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_MANIFEST_DIR" "crates/vox-orchestrator-types/build.rs"`

**Confidence**: high

---

### hv-0756 — `crates/vox-orchestrator-types/build.rs:27`

**Substring**

```text
OUT_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "OUT_DIR" "crates/vox-orchestrator-types/build.rs"`

**Confidence**: high

---

### hv-0757 — `crates/vox-orchestrator/build.rs:14`

**Substring**

```text
CARGO_MANIFEST_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CARGO_MANIFEST_DIR" "crates/vox-orchestrator/build.rs"`

**Confidence**: high

---

### hv-0758 — `crates/vox-orchestrator/build.rs:24`

**Substring**

```text
OUT_DIR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "OUT_DIR" "crates/vox-orchestrator/build.rs"`

**Confidence**: high

---

### hv-0759 — `crates/vox-orchestrator/src/populi_remote.rs:42`

**Substring**

```text
COMPUTERNAME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "COMPUTERNAME" "crates/vox-orchestrator/src/populi_remote.rs"`

**Confidence**: high

---

### hv-0760 — `crates/vox-orchestrator/src/populi_remote.rs:43`

**Substring**

```text
HOSTNAME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOSTNAME" "crates/vox-orchestrator/src/populi_remote.rs"`

**Confidence**: high

---

### hv-0761 — `crates/vox-populi/src/node_registry.rs:177`

**Substring**

```text
HOME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOME" "crates/vox-populi/src/node_registry.rs"`

**Confidence**: high

---

### hv-0762 — `crates/vox-populi/src/node_registry.rs:178`

**Substring**

```text
USERPROFILE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERPROFILE" "crates/vox-populi/src/node_registry.rs"`

**Confidence**: high

---

### hv-0763 — `crates/vox-repository/src/capabilities.rs:191`

**Substring**

```text
HOSTNAME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOSTNAME" "crates/vox-repository/src/capabilities.rs"`

**Confidence**: high

---

### hv-0764 — `crates/vox-repository/src/capabilities.rs:192`

**Substring**

```text
COMPUTERNAME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "COMPUTERNAME" "crates/vox-repository/src/capabilities.rs"`

**Confidence**: high

---

### hv-0765 — `crates/vox-secrets/src/backend/infisical.rs:17`

**Substring**

```text
INFISICAL_TOKEN
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "INFISICAL_TOKEN" "crates/vox-secrets/src/backend/infisical.rs"`

**Confidence**: high

---

### hv-0766 — `crates/vox-secrets/src/backend/infisical.rs:18`

**Substring**

```text
INFISICAL_SERVICE_TOKEN
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "INFISICAL_SERVICE_TOKEN" "crates/vox-secrets/src/backend/infisical.rs"`

**Confidence**: high

---

### hv-0767 — `crates/vox-secrets/src/backend/vault.rs:17`

**Substring**

```text
VAULT_ADDR
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "VAULT_ADDR" "crates/vox-secrets/src/backend/vault.rs"`

**Confidence**: high

---

### hv-0768 — `crates/vox-secrets/src/sources/auth_json.rs:26`

**Substring**

```text
HOME
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "HOME" "crates/vox-secrets/src/sources/auth_json.rs"`

**Confidence**: high

---

### hv-0769 — `crates/vox-secrets/src/sources/auth_json.rs:27`

**Substring**

```text
USERPROFILE
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "USERPROFILE" "crates/vox-secrets/src/sources/auth_json.rs"`

**Confidence**: high

---

### hv-0770 — `crates/vox-share/src/consent.rs:83`

**Substring**

```text
CI
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "CI" "crates/vox-share/src/consent.rs"`

**Confidence**: high

---

### hv-0771 — `crates/voxup/src/install.rs:91`

**Substring**

```text
PATH
```

**Why it matters**: Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.

**Fix** (register-env-and-use-secrets): Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "PATH" "crates/voxup/src/install.rs"`

**Confidence**: high

---

