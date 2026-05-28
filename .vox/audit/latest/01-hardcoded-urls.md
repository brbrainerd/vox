# 01 — hardcoded urls

**Severity**: warning  
**Itemized**: 100

### hv-0001 — `contracts/aci/agent-computer-interface-ssot.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/aci/agent-computer-interface-ssot.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/aci/agent-computer-interface-ssot.v1.schema.json\"" "contracts/aci/agent-computer-interface-ssot.v1.schema.json"`

**Confidence**: medium

---

### hv-0002 — `contracts/aci/agent-computer-interface.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/aci/agent-computer-interface.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/aci/agent-computer-interface.v1.schema.json\"" "contracts/aci/agent-computer-interface.v1.schema.json"`

**Confidence**: medium

---

### hv-0003 — `contracts/agentos/ai-first-fixtures.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/schemas/agentos/ai-first-fixtures.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/agentos/ai-first-fixtures.v1.schema.json\"" "contracts/agentos/ai-first-fixtures.v1.schema.json"`

**Confidence**: medium

---

### hv-0004 — `contracts/capability/capability-registry.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/capability-registry.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/capability-registry.schema.json\"" "contracts/capability/capability-registry.schema.json"`

**Confidence**: medium

---

### hv-0005 — `contracts/cli/command-registry.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/command-registry.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/command-registry.schema.json\"" "contracts/cli/command-registry.schema.json"`

**Confidence**: medium

---

### hv-0006 — `contracts/code-audit/rules.v1.schema.json:3`

**Substring**

```text
"https://vox.dev/contracts/code-audit/rules.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox.dev/contracts/code-audit/rules.v1.schema.json\"" "contracts/code-audit/rules.v1.schema.json"`

**Confidence**: medium

---

### hv-0007 — `contracts/communication/a2a-clarification-payload.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/communication/a2a-clarification-payload.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/communication/a2a-clarification-payload.schema.json\"" "contracts/communication/a2a-clarification-payload.schema.json"`

**Confidence**: medium

---

### hv-0008 — `contracts/communication/context-envelope.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/communication/context-envelope.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/communication/context-envelope.schema.json\"" "contracts/communication/context-envelope.schema.json"`

**Confidence**: medium

---

### hv-0009 — `contracts/communication/interruption-decision.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/communication/interruption-decision.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/communication/interruption-decision.schema.json\"" "contracts/communication/interruption-decision.schema.json"`

**Confidence**: medium

---

### hv-0010 — `contracts/communication/orchestrator-persistence-outbox.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/communication/orchestrator-persistence-outbox.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/communication/orchestrator-persistence-outbox.schema.json\"" "contracts/communication/orchestrator-persistence-outbox.schema.json"`

**Confidence**: medium

---

### hv-0011 — `contracts/communication/protocol-catalog.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/communication-protocol-catalog.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/communication-protocol-catalog.schema.json\"" "contracts/communication/protocol-catalog.schema.json"`

**Confidence**: medium

---

### hv-0012 — `contracts/config/env-vars.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/schemas/config/env-vars.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/config/env-vars.v1.schema.json\"" "contracts/config/env-vars.v1.schema.json"`

**Confidence**: medium

---

### hv-0013 — `contracts/db/data-storage-guard-report.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/schemas/db/data-storage-guard-report.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/db/data-storage-guard-report.v1.schema.json\"" "contracts/db/data-storage-guard-report.v1.schema.json"`

**Confidence**: medium

---

### hv-0014 — `contracts/db/data-storage-policy.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/schemas/db/data-storage-policy.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/db/data-storage-policy.v1.schema.json\"" "contracts/db/data-storage-policy.v1.schema.json"`

**Confidence**: medium

---

### hv-0015 — `contracts/dei/rpc-methods.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/dei/rpc-methods.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/dei/rpc-methods.schema.json\"" "contracts/dei/rpc-methods.schema.json"`

**Confidence**: medium

---

### hv-0016 — `contracts/documentation/canonical-map.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/schemas/canonical-map.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/canonical-map.v1.schema.json\"" "contracts/documentation/canonical-map.v1.schema.json"`

**Confidence**: medium

---

### hv-0017 — `contracts/eval/benchmark-matrix.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/eval/benchmark-matrix.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/eval/benchmark-matrix.schema.json\"" "contracts/eval/benchmark-matrix.schema.json"`

**Confidence**: medium

---

### hv-0018 — `contracts/eval/external-serving-handoff.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/eval/external-serving-handoff.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/eval/external-serving-handoff.schema.json\"" "contracts/eval/external-serving-handoff.schema.json"`

**Confidence**: medium

---

### hv-0019 — `contracts/eval/mens-scorecard-event.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/eval/mens-scorecard-event.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/eval/mens-scorecard-event.schema.json\"" "contracts/eval/mens-scorecard-event.schema.json"`

**Confidence**: medium

---

### hv-0020 — `contracts/eval/mens-scorecard-summary.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/eval/mens-scorecard-summary.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/eval/mens-scorecard-summary.schema.json\"" "contracts/eval/mens-scorecard-summary.schema.json"`

**Confidence**: medium

---

### hv-0021 — `contracts/eval/mens-scorecard-summary.schema.json:55`

**Substring**

```text
"https://vox-lang.org/schemas/eval/mens-scorecard-event.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/eval/mens-scorecard-event.schema.json\"" "contracts/eval/mens-scorecard-summary.schema.json"`

**Confidence**: medium

---

### hv-0022 — `contracts/eval/mens-scorecard.baseline.json:13`

**Substring**

```text
"http://127.0.0.1:8080"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"http://127.0.0.1:8080\"" "contracts/eval/mens-scorecard.baseline.json"`

**Confidence**: medium

---

### hv-0023 — `contracts/eval/mens-scorecard.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/eval/mens-scorecard.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/eval/mens-scorecard.schema.json\"" "contracts/eval/mens-scorecard.schema.json"`

**Confidence**: medium

---

### hv-0024 — `contracts/eval/runtime-generation-kpi.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/eval/runtime-generation-kpi.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/eval/runtime-generation-kpi.schema.json\"" "contracts/eval/runtime-generation-kpi.schema.json"`

**Confidence**: medium

---

### hv-0025 — `contracts/eval/syntax-k-event.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/eval/syntax-k-event.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/eval/syntax-k-event.schema.json\"" "contracts/eval/syntax-k-event.schema.json"`

**Confidence**: medium

---

### hv-0026 — `contracts/eval/vision-rubric-output.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/eval/vision-rubric-output.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/eval/vision-rubric-output.schema.json\"" "contracts/eval/vision-rubric-output.schema.json"`

**Confidence**: medium

---

### hv-0027 — `contracts/index.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/contracts-index.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/contracts-index.schema.json\"" "contracts/index.schema.json"`

**Confidence**: medium

---

### hv-0028 — `contracts/journeys/canonical-journey-definition.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/journeys/canonical-journey-definition.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/journeys/canonical-journey-definition.v1.schema.json\"" "contracts/journeys/canonical-journey-definition.v1.schema.json"`

**Confidence**: medium

---

### hv-0029 — `contracts/journeys/canonical-journey-step.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/journeys/canonical-journey-step.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/journeys/canonical-journey-step.v1.schema.json\"" "contracts/journeys/canonical-journey-step.v1.schema.json"`

**Confidence**: medium

---

### hv-0030 — `contracts/journeys/journey-limitation.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/journeys/journey-limitation.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/journeys/journey-limitation.v1.schema.json\"" "contracts/journeys/journey-limitation.v1.schema.json"`

**Confidence**: medium

---

### hv-0031 — `contracts/manifest/vox-bundle.v1.schema.json:3`

**Substring**

```text
"https://vox.dev/contracts/manifest/vox-bundle.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox.dev/contracts/manifest/vox-bundle.v1.schema.json\"" "contracts/manifest/vox-bundle.v1.schema.json"`

**Confidence**: medium

---

### hv-0032 — `contracts/mcp/http-read-role-governance.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/mcp/http-read-role-governance.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/mcp/http-read-role-governance.schema.json\"" "contracts/mcp/http-read-role-governance.schema.json"`

**Confidence**: medium

---

### hv-0033 — `contracts/mcp/tool-registry.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/mcp/tool-registry.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/mcp/tool-registry.schema.json\"" "contracts/mcp/tool-registry.schema.json"`

**Confidence**: medium

---

### hv-0034 — `contracts/mens/review-dataset.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/mens/review-dataset.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/mens/review-dataset.schema.json\"" "contracts/mens/review-dataset.schema.json"`

**Confidence**: medium

---

### hv-0035 — `contracts/mens/training-preflight.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/mens/training-preflight.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/mens/training-preflight.schema.json\"" "contracts/mens/training-preflight.schema.json"`

**Confidence**: medium

---

### hv-0036 — `contracts/openclaw/discovery/well-known.minimal.json:3`

**Substring**

```text
"http://127.0.0.1:3000"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"http://127.0.0.1:3000\"" "contracts/openclaw/discovery/well-known.minimal.json"`

**Confidence**: medium

---

### hv-0037 — `contracts/openclaw/discovery/well-known.response.json:4`

**Substring**

```text
"https://gateway.openclaw.example"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://gateway.openclaw.example\"" "contracts/openclaw/discovery/well-known.response.json"`

**Confidence**: medium

---

### hv-0038 — `contracts/openclaw/discovery/well-known.response.json:8`

**Substring**

```text
"https://gateway.openclaw.example/v1/skills"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://gateway.openclaw.example/v1/skills\"" "contracts/openclaw/discovery/well-known.response.json"`

**Confidence**: medium

---

### hv-0039 — `contracts/openclaw/discovery/well-known.response.json:9`

**Substring**

```text
"https://gateway.openclaw.example/v1/skills/search"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://gateway.openclaw.example/v1/skills/search\"" "contracts/openclaw/discovery/well-known.response.json"`

**Confidence**: medium

---

### hv-0040 — `contracts/operations/catalog.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/operations/catalog.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/operations/catalog.v1.schema.json\"" "contracts/operations/catalog.v1.schema.json"`

**Confidence**: medium

---

### hv-0041 — `contracts/operations/completion-policy.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/operations/completion-policy.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/operations/completion-policy.v1.schema.json\"" "contracts/operations/completion-policy.v1.schema.json"`

**Confidence**: medium

---

### hv-0042 — `contracts/orchestration/agent-harness.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/orchestration/agent-harness.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/orchestration/agent-harness.schema.json\"" "contracts/orchestration/agent-harness.schema.json"`

**Confidence**: medium

---

### hv-0043 — `contracts/orchestration/agent-vcs-facade.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/orchestration/agent-vcs-facade.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/orchestration/agent-vcs-facade.schema.json\"" "contracts/orchestration/agent-vcs-facade.schema.json"`

**Confidence**: medium

---

### hv-0044 — `contracts/orchestration/context-lifecycle-telemetry.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/orchestration/context-lifecycle-telemetry.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/orchestration/context-lifecycle-telemetry.schema.json\"" "contracts/orchestration/context-lifecycle-telemetry.schema.json"`

**Confidence**: medium

---

### hv-0045 — `contracts/orchestration/context-work-item.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/orchestration/context-work-item.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/orchestration/context-work-item.schema.json\"" "contracts/orchestration/context-work-item.schema.json"`

**Confidence**: medium

---

### hv-0046 — `contracts/orchestration/journey-envelope.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/orchestration/journey-envelope.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/orchestration/journey-envelope.v1.schema.json\"" "contracts/orchestration/journey-envelope.v1.schema.json"`

**Confidence**: medium

---

### hv-0047 — `contracts/orchestration/orch-daemon-rpc-methods.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/orchestration/orch-daemon-rpc-methods.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/orchestration/orch-daemon-rpc-methods.schema.json\"" "contracts/orchestration/orch-daemon-rpc-methods.schema.json"`

**Confidence**: medium

---

### hv-0048 — `contracts/orchestration/repo-reconstruction.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/orchestration/repo-reconstruction.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/orchestration/repo-reconstruction.schema.json\"" "contracts/orchestration/repo-reconstruction.schema.json"`

**Confidence**: medium

---

### hv-0049 — `contracts/orchestration/vox-generate-code-file-outcomes.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/orchestration/vox-generate-code-file-outcomes.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/orchestration/vox-generate-code-file-outcomes.schema.json\"" "contracts/orchestration/vox-generate-code-file-outcomes.schema.json"`

**Confidence**: medium

---

### hv-0050 — `contracts/proximity/retired-surfaces.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/retired-surfaces.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/retired-surfaces.schema.json\"" "contracts/proximity/retired-surfaces.schema.json"`

**Confidence**: medium

---

### hv-0051 — `contracts/reports/ai-fixture-holes/ledger.v1.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/reports/ai-fixture-holes/ledger.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/reports/ai-fixture-holes/ledger.v1.schema.json\"" "contracts/reports/ai-fixture-holes/ledger.v1.schema.json"`

**Confidence**: medium

---

### hv-0052 — `contracts/reports/research-eval/results.v1.schema.json:3`

**Substring**

```text
"https://vox.local/contracts/reports/research-eval/results.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox.local/contracts/reports/research-eval/results.v1.schema.json\"" "contracts/reports/research-eval/results.v1.schema.json"`

**Confidence**: medium

---

### hv-0053 — `contracts/reports/research/artifact.v1.schema.json:3`

**Substring**

```text
"https://vox.local/contracts/reports/research/artifact.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox.local/contracts/reports/research/artifact.v1.schema.json\"" "contracts/reports/research/artifact.v1.schema.json"`

**Confidence**: medium

---

### hv-0054 — `contracts/reports/scaling-audit/findings-array.v1.schema.json:4`

**Substring**

```text
"https://vox.local/contracts/reports/scaling-audit/findings-array.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox.local/contracts/reports/scaling-audit/findings-array.v1.schema.json\"" "contracts/reports/scaling-audit/findings-array.v1.schema.json"`

**Confidence**: medium

---

### hv-0055 — `contracts/reports/scaling-audit/findings-latest.json:17535`

**Substring**

```text
"https://api.github.com/repos/vox-foundation/vox/actions/runs?branch=main&event=push&per_page=10\"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://api.github.com/repos/vox-foundation/vox/actions/runs?branch=main&event=push&per_page=10\\\"" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: high

---

### hv-0056 — `contracts/reports/scaling-audit/findings-latest.json:20812`

**Substring**

```text
"https://api.github.com/repos/{owner}/{repo}/issues/{pr_number}/comments\"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://api.github.com/repos/{owner}/{repo}/issues/{pr_number}/comments\\\"" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: high

---

### hv-0057 — `contracts/reports/scaling-audit/findings-scaling-latest.json:2123`

**Substring**

```text
"https://api.github.com/repos/{owner}/{repo}/issues/{pr_number}/comments\"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://api.github.com/repos/{owner}/{repo}/issues/{pr_number}/comments\\\"" "contracts/reports/scaling-audit/findings-scaling-latest.json"`

**Confidence**: high

---

### hv-0058 — `contracts/reports/scientia-novelty-evidence-bundle.example.v1.json:11`

**Substring**

```text
"https://example.invalid/work/placeholder"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://example.invalid/work/placeholder\"" "contracts/reports/scientia-novelty-evidence-bundle.example.v1.json"`

**Confidence**: medium

---

### hv-0059 — `contracts/reports/toestub-remediation/delta-after-remediation.v1.schema.json:4`

**Substring**

```text
"https://vox.local/contracts/reports/toestub-remediation/delta-after-remediation.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox.local/contracts/reports/toestub-remediation/delta-after-remediation.v1.schema.json\"" "contracts/reports/toestub-remediation/delta-after-remediation.v1.schema.json"`

**Confidence**: medium

---

### hv-0060 — `contracts/repository/repo-catalog.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/repository/repo-catalog.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/repository/repo-catalog.schema.json\"" "contracts/repository/repo-catalog.schema.json"`

**Confidence**: medium

---

### hv-0061 — `contracts/repository/repo-path-resolution.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/repository/repo-path-resolution.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/repository/repo-path-resolution.schema.json\"" "contracts/repository/repo-path-resolution.schema.json"`

**Confidence**: medium

---

### hv-0062 — `contracts/repository/repo-workspace-status.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/repository/repo-workspace-status.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/repository/repo-workspace-status.schema.json\"" "contracts/repository/repo-workspace-status.schema.json"`

**Confidence**: medium

---

### hv-0063 — `contracts/repository/vox-project-scaffold-result.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/repository/vox-project-scaffold-result.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/repository/vox-project-scaffold-result.schema.json\"" "contracts/repository/vox-project-scaffold-result.schema.json"`

**Confidence**: medium

---

### hv-0064 — `contracts/rust/ecosystem-support.schema.json:3`

**Substring**

```text
"https://vox/contracts/rust/ecosystem-support.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox/contracts/rust/ecosystem-support.schema.json\"" "contracts/rust/ecosystem-support.schema.json"`

**Confidence**: medium

---

### hv-0065 — `contracts/scaling/policy.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/scaling-policy.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/scaling-policy.schema.json\"" "contracts/scaling/policy.schema.json"`

**Confidence**: medium

---

### hv-0066 — `contracts/scientia/arxiv-handoff.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/arxiv-handoff.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/arxiv-handoff.schema.json\"" "contracts/scientia/arxiv-handoff.schema.json"`

**Confidence**: medium

---

### hv-0067 — `contracts/scientia/canonical-publication-metadata.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/canonical-publication-metadata.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/canonical-publication-metadata.v1.schema.json\"" "contracts/scientia/canonical-publication-metadata.v1.schema.json"`

**Confidence**: medium

---

### hv-0068 — `contracts/scientia/discovery-signal.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/discovery-signal.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/discovery-signal.schema.json\"" "contracts/scientia/discovery-signal.schema.json"`

**Confidence**: medium

---

### hv-0069 — `contracts/scientia/distribution.schema.json:3`

**Substring**

```text
"https://vox/contracts/scientia/distribution.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox/contracts/scientia/distribution.schema.json\"" "contracts/scientia/distribution.schema.json"`

**Confidence**: medium

---

### hv-0070 — `contracts/scientia/evidence-pack.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/evidence-pack.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/evidence-pack.v1.schema.json\"" "contracts/scientia/evidence-pack.v1.schema.json"`

**Confidence**: medium

---

### hv-0071 — `contracts/scientia/finding-candidate.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/finding-candidate.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/finding-candidate.v1.schema.json\"" "contracts/scientia/finding-candidate.v1.schema.json"`

**Confidence**: medium

---

### hv-0072 — `contracts/scientia/machine-suggestion-block.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/machine-suggestion-block.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/machine-suggestion-block.schema.json\"" "contracts/scientia/machine-suggestion-block.schema.json"`

**Confidence**: medium

---

### hv-0073 — `contracts/scientia/manifest-completion.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/manifest-completion.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/manifest-completion.schema.json\"" "contracts/scientia/manifest-completion.schema.json"`

**Confidence**: medium

---

### hv-0074 — `contracts/scientia/novelty-evidence-bundle.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/novelty-evidence-bundle.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/novelty-evidence-bundle.v1.schema.json\"" "contracts/scientia/novelty-evidence-bundle.v1.schema.json"`

**Confidence**: medium

---

### hv-0075 — `contracts/scientia/operator-status-surface.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/operator-status-surface.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/operator-status-surface.v1.schema.json\"" "contracts/scientia/operator-status-surface.v1.schema.json"`

**Confidence**: medium

---

### hv-0076 — `contracts/scientia/publication-worthiness.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/publication-worthiness.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/publication-worthiness.schema.json\"" "contracts/scientia/publication-worthiness.schema.json"`

**Confidence**: medium

---

### hv-0077 — `contracts/scientia/research-mesh-intake.v1.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/research-mesh-intake.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/research-mesh-intake.v1.schema.json\"" "contracts/scientia/research-mesh-intake.v1.schema.json"`

**Confidence**: medium

---

### hv-0078 — `contracts/scientia/research-mesh-promoted-line.v1.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/research-mesh-promoted-line.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/research-mesh-promoted-line.v1.schema.json\"" "contracts/scientia/research-mesh-promoted-line.v1.schema.json"`

**Confidence**: medium

---

### hv-0079 — `contracts/scientia/research-snapshot.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/research-snapshot.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/research-snapshot.v1.schema.json\"" "contracts/scientia/research-snapshot.v1.schema.json"`

**Confidence**: medium

---

### hv-0080 — `contracts/scientia/scientia-evidence-graph.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/scientia-evidence-graph.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/scientia-evidence-graph.schema.json\"" "contracts/scientia/scientia-evidence-graph.schema.json"`

**Confidence**: medium

---

### hv-0081 — `contracts/scientia/worthiness-signals.v2.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/scientia/worthiness-signals.v2.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/scientia/worthiness-signals.v2.schema.json\"" "contracts/scientia/worthiness-signals.v2.schema.json"`

**Confidence**: medium

---

### hv-0082 — `contracts/speech-to-code/audit-matrix.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/speech-to-code/audit-matrix.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/speech-to-code/audit-matrix.schema.json\"" "contracts/speech-to-code/audit-matrix.schema.json"`

**Confidence**: medium

---

### hv-0083 — `contracts/speech-to-code/failure-taxonomy.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/speech-to-code/failure-taxonomy.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/speech-to-code/failure-taxonomy.schema.json\"" "contracts/speech-to-code/failure-taxonomy.schema.json"`

**Confidence**: medium

---

### hv-0084 — `contracts/speech-to-code/kpi-baseline.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/speech-to-code/kpi-baseline.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/speech-to-code/kpi-baseline.schema.json\"" "contracts/speech-to-code/kpi-baseline.schema.json"`

**Confidence**: medium

---

### hv-0085 — `contracts/speech-to-code/lexicon.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/speech-to-code/lexicon.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/speech-to-code/lexicon.schema.json\"" "contracts/speech-to-code/lexicon.schema.json"`

**Confidence**: medium

---

### hv-0086 — `contracts/speech-to-code/speech_trace.mens.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/speech-to-code/speech_trace.mens.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/speech-to-code/speech_trace.mens.schema.json\"" "contracts/speech-to-code/speech_trace.mens.schema.json"`

**Confidence**: medium

---

### hv-0087 — `contracts/speech-to-code/speech_trace.schema.json:3`

**Substring**

```text
"https://vox-lang.org/schemas/speech-to-code/speech_trace.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/speech-to-code/speech_trace.schema.json\"" "contracts/speech-to-code/speech_trace.schema.json"`

**Confidence**: medium

---

### hv-0088 — `contracts/telemetry/agentos-guardrail-deny.v1.schema.json:3`

**Substring**

```text
"https://vox-lang.org/contracts/telemetry/agentos-guardrail-deny.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/telemetry/agentos-guardrail-deny.v1.schema.json\"" "contracts/telemetry/agentos-guardrail-deny.v1.schema.json"`

**Confidence**: medium

---

### hv-0089 — `contracts/telemetry/completion-detector-snapshot.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/telemetry/completion-detector-snapshot.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/telemetry/completion-detector-snapshot.v1.schema.json\"" "contracts/telemetry/completion-detector-snapshot.v1.schema.json"`

**Confidence**: medium

---

### hv-0090 — `contracts/telemetry/completion-finding.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/telemetry/completion-finding.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/telemetry/completion-finding.v1.schema.json\"" "contracts/telemetry/completion-finding.v1.schema.json"`

**Confidence**: medium

---

### hv-0091 — `contracts/telemetry/completion-run.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/telemetry/completion-run.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/telemetry/completion-run.v1.schema.json\"" "contracts/telemetry/completion-run.v1.schema.json"`

**Confidence**: medium

---

### hv-0092 — `contracts/telemetry/events.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/schemas/telemetry/events.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/schemas/telemetry/events.v1.schema.json\"" "contracts/telemetry/events.v1.schema.json"`

**Confidence**: medium

---

### hv-0093 — `contracts/telemetry/fixture-hole-observed.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/telemetry/fixture-hole-observed.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/telemetry/fixture-hole-observed.v1.schema.json\"" "contracts/telemetry/fixture-hole-observed.v1.schema.json"`

**Confidence**: medium

---

### hv-0094 — `contracts/telemetry/fixture-model-intent-resolved.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/telemetry/fixture-model-intent-resolved.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/telemetry/fixture-model-intent-resolved.v1.schema.json\"" "contracts/telemetry/fixture-model-intent-resolved.v1.schema.json"`

**Confidence**: medium

---

### hv-0095 — `contracts/telemetry/fixture-prompt-dispatch.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/telemetry/fixture-prompt-dispatch.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/telemetry/fixture-prompt-dispatch.v1.schema.json\"" "contracts/telemetry/fixture-prompt-dispatch.v1.schema.json"`

**Confidence**: medium

---

### hv-0096 — `contracts/telemetry/fixture-search-dispatch.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/telemetry/fixture-search-dispatch.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/telemetry/fixture-search-dispatch.v1.schema.json\"" "contracts/telemetry/fixture-search-dispatch.v1.schema.json"`

**Confidence**: medium

---

### hv-0097 — `contracts/telemetry/orch-subagent-dispatch.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/telemetry/orch-subagent-dispatch.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/telemetry/orch-subagent-dispatch.v1.schema.json\"" "contracts/telemetry/orch-subagent-dispatch.v1.schema.json"`

**Confidence**: medium

---

### hv-0098 — `contracts/telemetry/research-event-bridge.v1.schema.json:3`

**Substring**

```text
"https://vox.dev/contracts/telemetry/research-event-bridge.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox.dev/contracts/telemetry/research-event-bridge.v1.schema.json\"" "contracts/telemetry/research-event-bridge.v1.schema.json"`

**Confidence**: medium

---

### hv-0099 — `contracts/telemetry/scientia-novelty-decision-calibration.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/telemetry/scientia-novelty-decision-calibration.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/telemetry/scientia-novelty-decision-calibration.v1.schema.json\"" "contracts/telemetry/scientia-novelty-decision-calibration.v1.schema.json"`

**Confidence**: medium

---

### hv-0100 — `contracts/terminal/exec-policy.v1.schema.json:4`

**Substring**

```text
"https://vox-lang.org/contracts/terminal/exec-policy.v1.schema.json"
```

**Why it matters**: Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.

**Fix** (register-env-and-use-secrets): // Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.

**SSOT**: `contracts/config/env-vars.v1.yaml`

**Verify**: `rg -nF "\"https://vox-lang.org/contracts/terminal/exec-policy.v1.schema.json\"" "contracts/terminal/exec-policy.v1.schema.json"`

**Confidence**: medium

---

