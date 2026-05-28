# 09 — hardcoded model names

**Severity**: warning  
**Itemized**: 60

### hv-0625 — `contracts/orchestration/model-catalog.bootstrap.v1.json:184`

**Substring**

```text
"claude-mythos-preview-20260407"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-mythos-preview-20260407\"" "contracts/orchestration/model-catalog.bootstrap.v1.json"`

**Confidence**: medium

---

### hv-0626 — `contracts/orchestration/model-routing.v1.yaml:72`

**Substring**

```text
"claude-mythos-preview-20260407"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-mythos-preview-20260407\"" "contracts/orchestration/model-routing.v1.yaml"`

**Confidence**: medium

---

### hv-0627 — `contracts/orchestration/model-routing.v1.yaml:73`

**Substring**

```text
"claude-mythos-preview-20260407"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-mythos-preview-20260407\"" "contracts/orchestration/model-routing.v1.yaml"`

**Confidence**: medium

---

### hv-0628 — `contracts/orchestration/model-routing.v1.yaml:74`

**Substring**

```text
"claude-mythos-preview-20260407"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-mythos-preview-20260407\"" "contracts/orchestration/model-routing.v1.yaml"`

**Confidence**: medium

---

### hv-0629 — `contracts/speech-to-code/audit-matrix.schema.json:36`

**Substring**

```text
"whisper"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper\"" "contracts/speech-to-code/audit-matrix.schema.json"`

**Confidence**: medium

---

### hv-0630 — `crates/vox-cli/src/commands/ci/retired_symbol_check.rs:298`

**Substring**

```text
"CLAUDE.md"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"CLAUDE.md\"" "crates/vox-cli/src/commands/ci/retired_symbol_check.rs"`

**Confidence**: medium

---

### hv-0631 — `crates/vox-cli/src/commands/ci/speech_runtime_suite.rs:154`

**Substring**

```text
"whisper"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper\"" "crates/vox-cli/src/commands/ci/speech_runtime_suite.rs"`

**Confidence**: medium

---

### hv-0632 — `crates/vox-code-audit/src/detectors/llm_provider_call.rs:208`

**Substring**

```text
"gpt-4"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"gpt-4\"" "crates/vox-code-audit/src/detectors/llm_provider_call.rs"`

**Confidence**: medium

---

### hv-0633 — `crates/vox-code-audit/src/review/providers.rs:72`

**Substring**

```text
"gpt-4o-mini"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"gpt-4o-mini\"" "crates/vox-code-audit/src/review/providers.rs"`

**Confidence**: medium

---

### hv-0634 — `crates/vox-codegen/src/codegen_ts/routes.rs:46`

**Substring**

```text
"claude-sonnet-4-20250514"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-sonnet-4-20250514\"" "crates/vox-codegen/src/codegen_ts/routes.rs"`

**Confidence**: medium

---

### hv-0635 — `crates/vox-compiler/src/typeck/builtins.rs:391`

**Substring**

```text
"Claude"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Claude\"" "crates/vox-compiler/src/typeck/builtins.rs"`

**Confidence**: medium

---

### hv-0636 — `crates/vox-compiler/src/typeck/builtins.rs:393`

**Substring**

```text
"ClaudeActor"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"ClaudeActor\"" "crates/vox-compiler/src/typeck/builtins.rs"`

**Confidence**: medium

---

### hv-0637 — `crates/vox-config/src/operator_registry.rs:341`

**Substring**

```text
"gpt-4o"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"gpt-4o\"" "crates/vox-config/src/operator_registry.rs"`

**Confidence**: medium

---

### hv-0638 — `crates/vox-config/src/operator_registry.rs:586`

**Substring**

```text
"text-embedding-3-small"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"text-embedding-3-small\"" "crates/vox-config/src/operator_registry.rs"`

**Confidence**: medium

---

### hv-0639 — `crates/vox-corpus/src/synthetic_gen/bodies/_tool_pairs_body.rs:135`

**Substring**

```text
"claude-3-5-haiku"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-3-5-haiku\"" "crates/vox-corpus/src/synthetic_gen/bodies/_tool_pairs_body.rs"`

**Confidence**: medium

---

### hv-0640 — `crates/vox-gamify/src/cost.rs:192`

**Substring**

```text
"claude-3"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-3\"" "crates/vox-gamify/src/cost.rs"`

**Confidence**: medium

---

### hv-0641 — `crates/vox-gamify/src/cost.rs:200`

**Substring**

```text
"claude-3"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-3\"" "crates/vox-gamify/src/cost.rs"`

**Confidence**: medium

---

### hv-0642 — `crates/vox-ml-cli/src/commands/mens/system_prompt_template.rs:34`

**Substring**

```text
"claude-code"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-code\"" "crates/vox-ml-cli/src/commands/mens/system_prompt_template.rs"`

**Confidence**: medium

---

### hv-0643 — `crates/vox-oratio/src/backend_dispatch.rs:40`

**Substring**

```text
"whisper"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper\"" "crates/vox-oratio/src/backend_dispatch.rs"`

**Confidence**: medium

---

### hv-0644 — `crates/vox-oratio/src/backends/candle_engine.rs:198`

**Substring**

```text
"whisper audio features"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper audio features\"" "crates/vox-oratio/src/backends/candle_engine.rs"`

**Confidence**: medium

---

### hv-0645 — `crates/vox-oratio/src/backends/candle_engine.rs:341`

**Substring**

```text
"whisper decode retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper decode retry\"" "crates/vox-oratio/src/backends/candle_engine.rs"`

**Confidence**: medium

---

### hv-0646 — `crates/vox-oratio/src/backends/candle_engine.rs:363`

**Substring**

```text
"whisper decode retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper decode retry\"" "crates/vox-oratio/src/backends/candle_engine.rs"`

**Confidence**: medium

---

### hv-0647 — `crates/vox-oratio/src/backends/candle_engine.rs:628`

**Substring**

```text
"whisper: no speech segment skipped"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper: no speech segment skipped\"" "crates/vox-oratio/src/backends/candle_engine.rs"`

**Confidence**: medium

---

### hv-0648 — `crates/vox-oratio/src/backends/candle_engine.rs:645`

**Substring**

```text
"whisper segment"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper segment\"" "crates/vox-oratio/src/backends/candle_engine.rs"`

**Confidence**: medium

---

### hv-0649 — `crates/vox-oratio/src/backends/candle_whisper.rs:179`

**Substring**

```text
"Whisper::load"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper::load\"" "crates/vox-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0650 — `crates/vox-oratio/src/backends/candle_whisper.rs:556`

**Substring**

```text
"Whisper model is busy; retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper model is busy; retry\"" "crates/vox-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0651 — `crates/vox-oratio/src/backends/candle_whisper.rs:626`

**Substring**

```text
"Whisper decoder init failed; session cleared — retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper decoder init failed; session cleared — retry\"" "crates/vox-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0652 — `crates/vox-oratio/src/backends/candle_whisper.rs:659`

**Substring**

```text
"Whisper inference"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper inference\"" "crates/vox-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0653 — `crates/vox-oratio/src/backends/candle_whisper.rs:689`

**Substring**

```text
"Whisper model is busy; retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper model is busy; retry\"" "crates/vox-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0654 — `crates/vox-oratio/src/backends/candle_whisper.rs:702`

**Substring**

```text
"Whisper decoder init failed; session cleared — retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper decoder init failed; session cleared — retry\"" "crates/vox-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0655 — `crates/vox-oratio/src/backends/candle_whisper.rs:745`

**Substring**

```text
"Whisper inference"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper inference\"" "crates/vox-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0656 — `crates/vox-oratio/src/backends/multilingual.rs:142`

**Substring**

```text
"whisper language candidate"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper language candidate\"" "crates/vox-oratio/src/backends/multilingual.rs"`

**Confidence**: medium

---

### hv-0657 — `crates/vox-oratio/src/refine/rules.rs:54`

**Substring**

```text
"whisper"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper\"" "crates/vox-oratio/src/refine/rules.rs"`

**Confidence**: medium

---

### hv-0658 — `crates/vox-orchestrator-mcp/src/vcs_tools/commit_tools.rs:126`

**Substring**

```text
"claude-opus-4-7"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-opus-4-7\"" "crates/vox-orchestrator-mcp/src/vcs_tools/commit_tools.rs"`

**Confidence**: medium

---

### hv-0659 — `crates/vox-orchestrator/src/events.rs:834`

**Substring**

```text
"claude-3"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-3\"" "crates/vox-orchestrator/src/events.rs"`

**Confidence**: medium

---

### hv-0660 — `crates/vox-orchestrator/src/models/tests.rs:40`

**Substring**

```text
"claude"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude\"" "crates/vox-orchestrator/src/models/tests.rs"`

**Confidence**: medium

---

### hv-0661 — `crates/vox-orchestrator/src/models/tests.rs:154`

**Substring**

```text
"claude-mythos-preview-20260407"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-mythos-preview-20260407\"" "crates/vox-orchestrator/src/models/tests.rs"`

**Confidence**: medium

---

### hv-0662 — `crates/vox-orchestrator/src/session/manager/tests.rs:88`

**Substring**

```text
"claude-sonnet-4"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-sonnet-4\"" "crates/vox-orchestrator/src/session/manager/tests.rs"`

**Confidence**: medium

---

### hv-0663 — `crates/vox-orchestrator/src/session/manager/tests.rs:91`

**Substring**

```text
"claude-sonnet-4"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-sonnet-4\"" "crates/vox-orchestrator/src/session/manager/tests.rs"`

**Confidence**: medium

---

### hv-0664 — `crates/vox-plugin-oratio/src/backends/candle_engine.rs:198`

**Substring**

```text
"whisper audio features"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper audio features\"" "crates/vox-plugin-oratio/src/backends/candle_engine.rs"`

**Confidence**: medium

---

### hv-0665 — `crates/vox-plugin-oratio/src/backends/candle_engine.rs:341`

**Substring**

```text
"whisper decode retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper decode retry\"" "crates/vox-plugin-oratio/src/backends/candle_engine.rs"`

**Confidence**: medium

---

### hv-0666 — `crates/vox-plugin-oratio/src/backends/candle_engine.rs:363`

**Substring**

```text
"whisper decode retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper decode retry\"" "crates/vox-plugin-oratio/src/backends/candle_engine.rs"`

**Confidence**: medium

---

### hv-0667 — `crates/vox-plugin-oratio/src/backends/candle_engine.rs:628`

**Substring**

```text
"whisper: no speech segment skipped"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper: no speech segment skipped\"" "crates/vox-plugin-oratio/src/backends/candle_engine.rs"`

**Confidence**: medium

---

### hv-0668 — `crates/vox-plugin-oratio/src/backends/candle_engine.rs:645`

**Substring**

```text
"whisper segment"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper segment\"" "crates/vox-plugin-oratio/src/backends/candle_engine.rs"`

**Confidence**: medium

---

### hv-0669 — `crates/vox-plugin-oratio/src/backends/candle_whisper.rs:173`

**Substring**

```text
"Whisper::load"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper::load\"" "crates/vox-plugin-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0670 — `crates/vox-plugin-oratio/src/backends/candle_whisper.rs:512`

**Substring**

```text
"Whisper model is busy; retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper model is busy; retry\"" "crates/vox-plugin-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0671 — `crates/vox-plugin-oratio/src/backends/candle_whisper.rs:582`

**Substring**

```text
"Whisper decoder init failed; session cleared — retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper decoder init failed; session cleared — retry\"" "crates/vox-plugin-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0672 — `crates/vox-plugin-oratio/src/backends/candle_whisper.rs:613`

**Substring**

```text
"Whisper inference"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper inference\"" "crates/vox-plugin-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0673 — `crates/vox-plugin-oratio/src/backends/candle_whisper.rs:643`

**Substring**

```text
"Whisper model is busy; retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper model is busy; retry\"" "crates/vox-plugin-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0674 — `crates/vox-plugin-oratio/src/backends/candle_whisper.rs:656`

**Substring**

```text
"Whisper decoder init failed; session cleared — retry"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper decoder init failed; session cleared — retry\"" "crates/vox-plugin-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0675 — `crates/vox-plugin-oratio/src/backends/candle_whisper.rs:685`

**Substring**

```text
"Whisper inference"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Whisper inference\"" "crates/vox-plugin-oratio/src/backends/candle_whisper.rs"`

**Confidence**: medium

---

### hv-0676 — `crates/vox-plugin-oratio/src/backends/multilingual.rs:142`

**Substring**

```text
"whisper language candidate"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"whisper language candidate\"" "crates/vox-plugin-oratio/src/backends/multilingual.rs"`

**Confidence**: medium

---

### hv-0677 — `crates/vox-plugin-publication/src/ingest.rs:34`

**Substring**

```text
"text-embedding-3-small"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"text-embedding-3-small\"" "crates/vox-plugin-publication/src/ingest.rs"`

**Confidence**: medium

---

### hv-0678 — `crates/vox-publisher/src/atlas/manifest.rs:87`

**Substring**

```text
"GPT-4o p95 latency increased by 120ms relative to GPT-4"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"GPT-4o p95 latency increased by 120ms relative to GPT-4\"" "crates/vox-publisher/src/atlas/manifest.rs"`

**Confidence**: medium

---

### hv-0679 — `crates/vox-research-events/src/observation.rs:113`

**Substring**

```text
"gpt-4o"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"gpt-4o\"" "crates/vox-research-events/src/observation.rs"`

**Confidence**: medium

---

### hv-0680 — `crates/vox-ro-crate/src/ai_disclosure.rs:65`

**Substring**

```text
"Claude"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Claude\"" "crates/vox-ro-crate/src/ai_disclosure.rs"`

**Confidence**: medium

---

### hv-0681 — `crates/vox-ro-crate/src/ai_disclosure.rs:71`

**Substring**

```text
"Claude"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"Claude\"" "crates/vox-ro-crate/src/ai_disclosure.rs"`

**Confidence**: medium

---

### hv-0682 — `crates/vox-search/src/embedding_env.rs:47`

**Substring**

```text
"text-embedding-3-small"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"text-embedding-3-small\"" "crates/vox-search/src/embedding_env.rs"`

**Confidence**: medium

---

### hv-0683 — `crates/vox-search/src/embedding_env.rs:72`

**Substring**

```text
"text-embedding-3-small"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"text-embedding-3-small\"" "crates/vox-search/src/embedding_env.rs"`

**Confidence**: medium

---

### hv-0684 — `crates/vox-telemetry/src/types.rs:514`

**Substring**

```text
"claude-opus-4-7"
```

**Why it matters**: Model ids should follow registry / user config to avoid lock-in to one provider spelling.

**Fix** (centralize-in-contract): Resolve model id from runtime manifest / capability registry / user config instead of string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"claude-opus-4-7\"" "crates/vox-telemetry/src/types.rs"`

**Confidence**: medium

---

