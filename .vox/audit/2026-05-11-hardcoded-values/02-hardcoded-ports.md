# 02 — hardcoded ports

**Severity**: warning  
**Itemized**: 73

### hv-0101 — `apps/editor/vox-vscode/src/extension.ts:44`

**Substring**

```text
http://127.0.0.1:3921
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:3921" "apps/editor/vox-vscode/src/extension.ts"`

**Confidence**: medium

---

### hv-0102 — `contracts/codex-api.openapi.yaml:10`

**Substring**

```text
http://127.0.0.1:3847
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:3847" "contracts/codex-api.openapi.yaml"`

**Confidence**: medium

---

### hv-0103 — `contracts/config/env-vars.v1.yaml:4107`

**Substring**

```text
http://127.0.0.1:7863
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:7863" "contracts/config/env-vars.v1.yaml"`

**Confidence**: medium

---

### hv-0104 — `contracts/eval/mens-scorecard.baseline.json:13`

**Substring**

```text
http://127.0.0.1:8080
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:8080" "contracts/eval/mens-scorecard.baseline.json"`

**Confidence**: medium

---

### hv-0105 — `contracts/mcp/http-gateway.openapi.yaml:16`

**Substring**

```text
http://127.0.0.1:3921
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:3921" "contracts/mcp/http-gateway.openapi.yaml"`

**Confidence**: medium

---

### hv-0106 — `contracts/openclaw/discovery/well-known.minimal.json:3`

**Substring**

```text
http://127.0.0.1:3000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:3000" "contracts/openclaw/discovery/well-known.minimal.json"`

**Confidence**: medium

---

### hv-0107 — `contracts/reports/scaling-audit/findings-latest.json:4432`

**Substring**

```text
http://127.0.0.1:9847
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:9847" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0108 — `contracts/reports/scaling-audit/findings-latest.json:4615`

**Substring**

```text
http://127.0.0.1:9847
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:9847" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0109 — `crates/vox-actor-runtime/src/builtins/tests.rs:224`

**Substring**

```text
http://127.0.0.1:1
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:1" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: medium

---

### hv-0110 — `crates/vox-actor-runtime/src/inference_env.rs:254`

**Substring**

```text
http://127.0.0.1:1
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:1" "crates/vox-actor-runtime/src/inference_env.rs"`

**Confidence**: medium

---

### hv-0111 — `crates/vox-actor-runtime/src/llm/cascade.rs:182`

**Substring**

```text
http://localhost:9999
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:9999" "crates/vox-actor-runtime/src/llm/cascade.rs"`

**Confidence**: medium

---

### hv-0112 — `crates/vox-actor-runtime/src/llm/cascade.rs:191`

**Substring**

```text
http://localhost:9999
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:9999" "crates/vox-actor-runtime/src/llm/cascade.rs"`

**Confidence**: medium

---

### hv-0113 — `crates/vox-actor-runtime/src/model_resolution.rs:324`

**Substring**

```text
http://localhost:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:11434" "crates/vox-actor-runtime/src/model_resolution.rs"`

**Confidence**: medium

---

### hv-0114 — `crates/vox-actor-runtime/src/model_resolution.rs:377`

**Substring**

```text
http://127.0.0.1:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:11434" "crates/vox-actor-runtime/src/model_resolution.rs"`

**Confidence**: medium

---

### hv-0115 — `crates/vox-actor-runtime/src/model_resolution.rs:383`

**Substring**

```text
http://127.0.0.1:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:11434" "crates/vox-actor-runtime/src/model_resolution.rs"`

**Confidence**: medium

---

### hv-0116 — `crates/vox-actor-runtime/src/model_resolution.rs:500`

**Substring**

```text
http://127.0.0.1:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:11434" "crates/vox-actor-runtime/src/model_resolution.rs"`

**Confidence**: medium

---

### hv-0117 — `crates/vox-cli/src/commands/bundle.rs:194`

**Substring**

```text
http://localhost:3000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:3000" "crates/vox-cli/src/commands/bundle.rs"`

**Confidence**: medium

---

### hv-0118 — `crates/vox-cli/src/commands/ci/pm_provenance.rs:123`

**Substring**

```text
http://localhost:0
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:0" "crates/vox-cli/src/commands/ci/pm_provenance.rs"`

**Confidence**: medium

---

### hv-0119 — `crates/vox-cli/src/commands/init.rs:75`

**Substring**

```text
http://localhost:3001
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:3001" "crates/vox-cli/src/commands/init.rs"`

**Confidence**: medium

---

### hv-0120 — `crates/vox-cli/src/commands/init.rs:80`

**Substring**

```text
http://localhost:3000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:3000" "crates/vox-cli/src/commands/init.rs"`

**Confidence**: medium

---

### hv-0121 — `crates/vox-cli/src/commands/init.rs:85`

**Substring**

```text
http://localhost:3001
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:3001" "crates/vox-cli/src/commands/init.rs"`

**Confidence**: medium

---

### hv-0122 — `crates/vox-cli/src/commands/init.rs:91`

**Substring**

```text
http://localhost:3000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:3000" "crates/vox-cli/src/commands/init.rs"`

**Confidence**: medium

---

### hv-0123 — `crates/vox-cli/src/commands/research/infra.rs:12`

**Substring**

```text
http://localhost:8080
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:8080" "crates/vox-cli/src/commands/research/infra.rs"`

**Confidence**: medium

---

### hv-0124 — `crates/vox-cli/src/commands/research/infra.rs:33`

**Substring**

```text
http://localhost:8080
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:8080" "crates/vox-cli/src/commands/research/infra.rs"`

**Confidence**: medium

---

### hv-0125 — `crates/vox-cli/src/frontend.rs:57`

**Substring**

```text
http://127.0.0.1:3001
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:3001" "crates/vox-cli/src/frontend.rs"`

**Confidence**: medium

---

### hv-0126 — `crates/vox-code-audit/src/ai_analyze.rs:59`

**Substring**

```text
http://localhost:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:11434" "crates/vox-code-audit/src/ai_analyze.rs"`

**Confidence**: medium

---

### hv-0127 — `crates/vox-code-audit/src/ai_analyze.rs:319`

**Substring**

```text
http://localhost:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:11434" "crates/vox-code-audit/src/ai_analyze.rs"`

**Confidence**: medium

---

### hv-0128 — `crates/vox-code-audit/src/ai_analyze.rs:324`

**Substring**

```text
http://localhost:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:11434" "crates/vox-code-audit/src/ai_analyze.rs"`

**Confidence**: medium

---

### hv-0129 — `crates/vox-code-audit/src/detectors/magic_value.rs:142`

**Substring**

```text
"127.0.0.1:0"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1:0\"" "crates/vox-code-audit/src/detectors/magic_value.rs"`

**Confidence**: medium

---

### hv-0130 — `crates/vox-code-audit/src/detectors/magic_value.rs:143`

**Substring**

```text
"0.0.0.0:0"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"0.0.0.0:0\"" "crates/vox-code-audit/src/detectors/magic_value.rs"`

**Confidence**: medium

---

### hv-0131 — `crates/vox-code-audit/src/detectors/magic_value.rs:144`

**Substring**

```text
"localhost:0"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"localhost:0\"" "crates/vox-code-audit/src/detectors/magic_value.rs"`

**Confidence**: medium

---

### hv-0132 — `crates/vox-code-audit/src/detectors/magic_value.rs:213`

**Substring**

```text
"127.0.0.1:3000"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1:3000\"" "crates/vox-code-audit/src/detectors/magic_value.rs"`

**Confidence**: medium

---

### hv-0133 — `crates/vox-code-audit/src/detectors/magic_value.rs:241`

**Substring**

```text
"127.0.0.1:5432"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1:5432\"" "crates/vox-code-audit/src/detectors/magic_value.rs"`

**Confidence**: medium

---

### hv-0134 — `crates/vox-code-audit/src/review/providers.rs:85`

**Substring**

```text
http://localhost:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:11434" "crates/vox-code-audit/src/review/providers.rs"`

**Confidence**: medium

---

### hv-0135 — `crates/vox-codegen/src/codegen_ts/scaffold.rs:108`

**Substring**

```text
http://127.0.0.1:4000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:4000" "crates/vox-codegen/src/codegen_ts/scaffold.rs"`

**Confidence**: medium

---

### hv-0136 — `crates/vox-config/src/inference.rs:234`

**Substring**

```text
http://localhost:9999
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:9999" "crates/vox-config/src/inference.rs"`

**Confidence**: medium

---

### hv-0137 — `crates/vox-config/src/inference.rs:236`

**Substring**

```text
http://localhost:9999
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:9999" "crates/vox-config/src/inference.rs"`

**Confidence**: medium

---

### hv-0138 — `crates/vox-config/src/operator_registry.rs:68`

**Substring**

```text
http://localhost:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:11434" "crates/vox-config/src/operator_registry.rs"`

**Confidence**: medium

---

### hv-0139 — `crates/vox-config/src/operator_registry.rs:75`

**Substring**

```text
http://localhost:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:11434" "crates/vox-config/src/operator_registry.rs"`

**Confidence**: medium

---

### hv-0140 — `crates/vox-config/src/operator_registry.rs:180`

**Substring**

```text
http://localhost:3000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:3000" "crates/vox-config/src/operator_registry.rs"`

**Confidence**: medium

---

### hv-0141 — `crates/vox-config/src/operator_registry.rs:306`

**Substring**

```text
http://localhost:8000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:8000" "crates/vox-config/src/operator_registry.rs"`

**Confidence**: medium

---

### hv-0142 — `crates/vox-openclaw-runtime/src/openclaw_adapter.rs:36`

**Substring**

```text
http://127.0.0.1:3000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:3000" "crates/vox-openclaw-runtime/src/openclaw_adapter.rs"`

**Confidence**: medium

---

### hv-0143 — `crates/vox-openclaw-runtime/src/openclaw_discovery.rs:274`

**Substring**

```text
http://127.0.0.1:3000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:3000" "crates/vox-openclaw-runtime/src/openclaw_discovery.rs"`

**Confidence**: medium

---

### hv-0144 — `crates/vox-openclaw-runtime/src/openclaw_discovery.rs:278`

**Substring**

```text
http://127.0.0.1:3000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:3000" "crates/vox-openclaw-runtime/src/openclaw_discovery.rs"`

**Confidence**: medium

---

### hv-0145 — `crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs:105`

**Substring**

```text
http://localhost:3000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://localhost:3000" "crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs"`

**Confidence**: high

---

### hv-0146 — `crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs:109`

**Substring**

```text
https://127.0.0.1:8080
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "https://127.0.0.1:8080" "crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs"`

**Confidence**: high

---

### hv-0147 — `crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs:121`

**Substring**

```text
"localhost:8080"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"localhost:8080\"" "crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs"`

**Confidence**: high

---

### hv-0148 — `crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs:127`

**Substring**

```text
"localhost:8080"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"localhost:8080\"" "crates/vox-orchestrator-mcp/src/http_gateway/origin_guard.rs"`

**Confidence**: high

---

### hv-0149 — `crates/vox-orchestrator-mcp/src/llm_bridge/mod.rs:64`

**Substring**

```text
http://127.0.0.1:7863
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:7863" "crates/vox-orchestrator-mcp/src/llm_bridge/mod.rs"`

**Confidence**: medium

---

### hv-0150 — `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs:186`

**Substring**

```text
http://127.0.0.1:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:11434" "crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs"`

**Confidence**: medium

---

### hv-0151 — `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs:325`

**Substring**

```text
http://127.0.0.1:11434
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:11434" "crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs"`

**Confidence**: medium

---

### hv-0152 — `crates/vox-orchestrator-mcp/src/llm_bridge/provider_endpoints.rs:81`

**Substring**

```text
http://127.0.0.1:7863
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:7863" "crates/vox-orchestrator-mcp/src/llm_bridge/provider_endpoints.rs"`

**Confidence**: medium

---

### hv-0153 — `crates/vox-orchestrator-mcp/src/llm_bridge/providers/probe.rs:18`

**Substring**

```text
http://127.0.0.1:7863
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:7863" "crates/vox-orchestrator-mcp/src/llm_bridge/providers/probe.rs"`

**Confidence**: medium

---

### hv-0154 — `crates/vox-orchestrator/src/config/tests.rs:108`

**Substring**

```text
http://127.0.0.1:11435
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:11435" "crates/vox-orchestrator/src/config/tests.rs"`

**Confidence**: medium

---

### hv-0155 — `crates/vox-orchestrator/src/config/tests.rs:122`

**Substring**

```text
http://127.0.0.1:11435
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:11435" "crates/vox-orchestrator/src/config/tests.rs"`

**Confidence**: medium

---

### hv-0156 — `crates/vox-orchestrator/src/mesh.rs:252`

**Substring**

```text
"localhost:5173"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"localhost:5173\"" "crates/vox-orchestrator/src/mesh.rs"`

**Confidence**: medium

---

### hv-0157 — `crates/vox-plugin-populi-mesh/src/mesh.rs:27`

**Substring**

```text
"127.0.0.1:9847"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1:9847\"" "crates/vox-plugin-populi-mesh/src/mesh.rs"`

**Confidence**: medium

---

### hv-0158 — `crates/vox-plugin-populi-mesh/src/transport/mod.rs:533`

**Substring**

```text
http://127.0.0.1:9847
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:9847" "crates/vox-plugin-populi-mesh/src/transport/mod.rs"`

**Confidence**: medium

---

### hv-0159 — `crates/vox-plugin-webhook/src/lib.rs:63`

**Substring**

```text
"0.0.0.0:9080"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"0.0.0.0:9080\"" "crates/vox-plugin-webhook/src/lib.rs"`

**Confidence**: medium

---

### hv-0160 — `crates/vox-populi/src/lib.rs:404`

**Substring**

```text
http://0.0.0.0:9847
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://0.0.0.0:9847" "crates/vox-populi/src/lib.rs"`

**Confidence**: medium

---

### hv-0161 — `crates/vox-populi/src/lib.rs:415`

**Substring**

```text
http://127.0.0.1:9847
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:9847" "crates/vox-populi/src/lib.rs"`

**Confidence**: medium

---

### hv-0162 — `crates/vox-populi/src/lib.rs:416`

**Substring**

```text
http://127.0.0.1:9847
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:9847" "crates/vox-populi/src/lib.rs"`

**Confidence**: medium

---

### hv-0163 — `crates/vox-populi/src/mens/cloud/local_provider.rs:60`

**Substring**

```text
"127.0.0.1:0"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1:0\"" "crates/vox-populi/src/mens/cloud/local_provider.rs"`

**Confidence**: high

---

### hv-0164 — `crates/vox-populi/src/transport/mod.rs:688`

**Substring**

```text
http://127.0.0.1:9847
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:9847" "crates/vox-populi/src/transport/mod.rs"`

**Confidence**: medium

---

### hv-0165 — `crates/vox-repository/src/populi_toml.rs:140`

**Substring**

```text
http://127.0.0.1:9999
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:9999" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0166 — `crates/vox-repository/src/populi_toml.rs:148`

**Substring**

```text
http://127.0.0.1:9999
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:9999" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0167 — `crates/vox-repository/src/populi_toml.rs:162`

**Substring**

```text
http://127.0.0.1:10000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:10000" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0168 — `crates/vox-repository/src/populi_toml.rs:164`

**Substring**

```text
http://127.0.0.1:11435
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:11435" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0169 — `crates/vox-repository/src/populi_toml.rs:169`

**Substring**

```text
http://127.0.0.1:10000
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:10000" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0170 — `crates/vox-repository/src/populi_toml.rs:173`

**Substring**

```text
http://127.0.0.1:11435
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:11435" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0171 — `crates/vox-repository/src/populi_toml.rs:209`

**Substring**

```text
http://127.0.0.1:9999
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "http://127.0.0.1:9999" "crates/vox-repository/src/populi_toml.rs"`

**Confidence**: medium

---

### hv-0172 — `crates/vox-share/src/backends/lan.rs:55`

**Substring**

```text
"0.0.0.0:0"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"0.0.0.0:0\"" "crates/vox-share/src/backends/lan.rs"`

**Confidence**: high

---

### hv-0173 — `crates/vox-test-harness/src/portpicker.rs:8`

**Substring**

```text
"127.0.0.1:0"
```

**Why it matters**: Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.

**Fix** (extract-named-constant): const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var("VOX_…_PORT") after registering in env-vars SSOT

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"127.0.0.1:0\"" "crates/vox-test-harness/src/portpicker.rs"`

**Confidence**: high

---

