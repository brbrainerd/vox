# 04 — hardcoded filesystem paths

**Severity**: warning  
**Itemized**: 20

### hv-0262 — `contracts/reports/scaling-audit/findings-latest.json:10191`

**Substring**

```text
"C:\\
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"C:\\\\" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0263 — `contracts/scaling/policy.yaml:17`

**Substring**

```text
"/tmp/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/tmp/" "contracts/scaling/policy.yaml"`

**Confidence**: medium

---

### hv-0264 — `crates/vox-cli/src/commands/ci/dev_loop_audit.rs:180`

**Substring**

```text
"/tmp/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/tmp/" "crates/vox-cli/src/commands/ci/dev_loop_audit.rs"`

**Confidence**: medium

---

### hv-0265 — `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs:254`

**Substring**

```text
"/usr/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/usr/" "crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs"`

**Confidence**: medium

---

### hv-0266 — `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs:295`

**Substring**

```text
"~/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"~/" "crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs"`

**Confidence**: medium

---

### hv-0267 — `crates/vox-code-audit/src/detectors/magic_value.rs:170`

**Substring**

```text
"/usr/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/usr/" "crates/vox-code-audit/src/detectors/magic_value.rs"`

**Confidence**: medium

---

### hv-0268 — `crates/vox-compiler/src/fmt/mod.rs:60`

**Substring**

```text
"/etc/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/etc/" "crates/vox-compiler/src/fmt/mod.rs"`

**Confidence**: medium

---

### hv-0269 — `crates/vox-compiler/src/fmt/mod.rs:74`

**Substring**

```text
"/etc/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/etc/" "crates/vox-compiler/src/fmt/mod.rs"`

**Confidence**: medium

---

### hv-0270 — `crates/vox-compiler/src/typeck/effect_check.rs:694`

**Substring**

```text
"/etc/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/etc/" "crates/vox-compiler/src/typeck/effect_check.rs"`

**Confidence**: medium

---

### hv-0271 — `crates/vox-config/src/operator_registry.rs:831`

**Substring**

```text
"~/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"~/" "crates/vox-config/src/operator_registry.rs"`

**Confidence**: medium

---

### hv-0272 — `crates/vox-config/src/operator_registry.rs:845`

**Substring**

```text
"C:\\
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"C:\\\\" "crates/vox-config/src/operator_registry.rs"`

**Confidence**: medium

---

### hv-0273 — `crates/vox-exec-grammar/src/ast.rs:453`

**Substring**

```text
"C:\\
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"C:\\\\" "crates/vox-exec-grammar/src/ast.rs"`

**Confidence**: medium

---

### hv-0274 — `crates/vox-ml-cli/src/commands/mens/pipeline.rs:187`

**Substring**

```text
"~/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"~/" "crates/vox-ml-cli/src/commands/mens/pipeline.rs"`

**Confidence**: medium

---

### hv-0275 — `crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs:174`

**Substring**

```text
"/tmp/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/tmp/" "crates/vox-plugin-mens-candle-cuda/src/checkpoint_state.rs"`

**Confidence**: medium

---

### hv-0276 — `crates/vox-plugin-mens-candle-metal/src/checkpoint_state.rs:174`

**Substring**

```text
"/tmp/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/tmp/" "crates/vox-plugin-mens-candle-metal/src/checkpoint_state.rs"`

**Confidence**: medium

---

### hv-0277 — `crates/vox-populi/src/mens/tensor/checkpoint_state.rs:170`

**Substring**

```text
"/tmp/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/tmp/" "crates/vox-populi/src/mens/tensor/checkpoint_state.rs"`

**Confidence**: medium

---

### hv-0278 — `crates/vox-repository/src/populi_toml.rs:211`

**Substring**

```text
"/etc/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/etc/" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0279 — `crates/vox-repository/src/populi_toml.rs:212`

**Substring**

```text
"/etc/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/etc/" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0280 — `crates/vox-repository/src/populi_toml.rs:221`

**Substring**

```text
"/etc/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/etc/" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0281 — `crates/vox-secrets/src/sources/auth_json.rs:196`

**Substring**

```text
"/tmp/
```

**Why it matters**: Absolute or home-relative paths fail cross-platform and on CI sandboxes.

**Fix** (use-config-path): Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\ or /home paths.

**SSOT**: `docs/src/architecture/data-storage-ssot-2026.md`

**Verify**: `rg -nF "\"/tmp/" "crates/vox-secrets/src/sources/auth_json.rs"`

**Confidence**: medium

---

