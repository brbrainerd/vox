# 14 — hardcoded date literals

**Severity**: info  
**Itemized**: 57

### hv-1032 — `contracts/reports/completion-audit.v1.json:4`

**Substring**

```text
2026-04-02
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-04-02" "contracts/reports/completion-audit.v1.json"`

**Confidence**: medium

---

### hv-1033 — `contracts/reports/docs-reality-audit/metrics.v1.json:8`

**Substring**

```text
2026-05-11
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-11" "contracts/reports/docs-reality-audit/metrics.v1.json"`

**Confidence**: medium

---

### hv-1034 — `contracts/reports/scaling-audit/audit-parse-latest.json:2`

**Substring**

```text
2026-03-25
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-25" "contracts/reports/scaling-audit/audit-parse-latest.json"`

**Confidence**: medium

---

### hv-1035 — `contracts/reports/scaling-audit/findings-latest.json:39981`

**Substring**

```text
2025-01-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2025-01-01" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-1036 — `contracts/reports/sql-write-ownership-rev-c.json:4`

**Substring**

```text
2026-03-25
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-25" "contracts/reports/sql-write-ownership-rev-c.json"`

**Confidence**: medium

---

### hv-1037 — `contracts/reports/toestub-remediation/baseline-freeze.json:3`

**Substring**

```text
2026-03-25
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-25" "contracts/reports/toestub-remediation/baseline-freeze.json"`

**Confidence**: medium

---

### hv-1038 — `contracts/reports/toestub-remediation/delta-after-remediation.json:2`

**Substring**

```text
2026-03-25
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-25" "contracts/reports/toestub-remediation/delta-after-remediation.json"`

**Confidence**: medium

---

### hv-1039 — `contracts/reports/toestub-remediation/promotion-metrics.json:89`

**Substring**

```text
2026-05-08
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-08" "contracts/reports/toestub-remediation/promotion-metrics.json"`

**Confidence**: medium

---

### hv-1040 — `contracts/reports/toestub-remediation/promotion-metrics.json:113`

**Substring**

```text
2026-03-25
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-25" "contracts/reports/toestub-remediation/promotion-metrics.json"`

**Confidence**: medium

---

### hv-1041 — `contracts/scaling/policy.yaml:4`

**Substring**

```text
2025-03-25
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2025-03-25" "contracts/scaling/policy.yaml"`

**Confidence**: medium

---

### hv-1042 — `contracts/speech-to-code/audit-matrix.v1.yaml:2`

**Substring**

```text
2026-05-11
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-11" "contracts/speech-to-code/audit-matrix.v1.yaml"`

**Confidence**: medium

---

### hv-1043 — `contracts/speech-to-code/canary.kpi.json:3`

**Substring**

```text
2026-05-11
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-11" "contracts/speech-to-code/canary.kpi.json"`

**Confidence**: medium

---

### hv-1044 — `contracts/speech-to-code/canary.kpi.json:4`

**Substring**

```text
2026-05-11
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-11" "contracts/speech-to-code/canary.kpi.json"`

**Confidence**: medium

---

### hv-1045 — `crates/vox-cli/src/commands/ci/nomenclature_guard.rs:40`

**Substring**

```text
2026-05-08
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-08" "crates/vox-cli/src/commands/ci/nomenclature_guard.rs"`

**Confidence**: medium

---

### hv-1046 — `crates/vox-cli/src/commands/ci/retired_symbol_check.rs:141`

**Substring**

```text
2026-05-08
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-08" "crates/vox-cli/src/commands/ci/retired_symbol_check.rs"`

**Confidence**: medium

---

### hv-1047 — `crates/vox-cli/src/commands/ci/retired_symbol_check.rs:142`

**Substring**

```text
2026-05-08
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-08" "crates/vox-cli/src/commands/ci/retired_symbol_check.rs"`

**Confidence**: medium

---

### hv-1048 — `crates/vox-cli/src/commands/ci/retired_symbol_check.rs:277`

**Substring**

```text
2026-05-08
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-08" "crates/vox-cli/src/commands/ci/retired_symbol_check.rs"`

**Confidence**: medium

---

### hv-1049 — `crates/vox-cli/src/commands/ci/test_inventory.rs:1184`

**Substring**

```text
2026-12-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-12-01" "crates/vox-cli/src/commands/ci/test_inventory.rs"`

**Confidence**: medium

---

### hv-1050 — `crates/vox-cli/src/commands/ci/test_inventory.rs:1185`

**Substring**

```text
2030-01-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2030-01-01" "crates/vox-cli/src/commands/ci/test_inventory.rs"`

**Confidence**: medium

---

### hv-1051 — `crates/vox-cli/src/commands/review/coderabbit/github/comments.rs:31`

**Substring**

```text
2022-11-28
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2022-11-28" "crates/vox-cli/src/commands/review/coderabbit/github/comments.rs"`

**Confidence**: medium

---

### hv-1052 — `crates/vox-cli/src/commands/review/coderabbit/ingest.rs:127`

**Substring**

```text
2022-11-28
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2022-11-28" "crates/vox-cli/src/commands/review/coderabbit/ingest.rs"`

**Confidence**: medium

---

### hv-1053 — `crates/vox-cli/src/commands/review/coderabbit/run_state.rs:83`

**Substring**

```text
2026-01-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-01-01" "crates/vox-cli/src/commands/review/coderabbit/run_state.rs"`

**Confidence**: medium

---

### hv-1054 — `crates/vox-codegen/src/codegen_rust/emit/client.rs:374`

**Substring**

```text
2024-11-05
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2024-11-05" "crates/vox-codegen/src/codegen_rust/emit/client.rs"`

**Confidence**: medium

---

### hv-1055 — `crates/vox-codegen/src/codegen_ts/routes.rs:43`

**Substring**

```text
2023-06-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2023-06-01" "crates/vox-codegen/src/codegen_ts/routes.rs"`

**Confidence**: medium

---

### hv-1056 — `crates/vox-db/src/codex_chat.rs:608`

**Substring**

```text
2026-03-21
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-21" "crates/vox-db/src/codex_chat.rs"`

**Confidence**: medium

---

### hv-1057 — `crates/vox-db/src/schema/domains/vox_mesh.rs:29`

**Substring**

```text
2026-05-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-01" "crates/vox-db/src/schema/domains/vox_mesh.rs"`

**Confidence**: medium

---

### hv-1058 — `crates/vox-doc-pipeline/src/pipeline/lint.rs:580`

**Substring**

```text
2026-05-08
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-08" "crates/vox-doc-pipeline/src/pipeline/lint.rs"`

**Confidence**: medium

---

### hv-1059 — `crates/vox-doc-pipeline/src/pipeline/lint.rs:583`

**Substring**

```text
2026-05-08
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-08" "crates/vox-doc-pipeline/src/pipeline/lint.rs"`

**Confidence**: medium

---

### hv-1060 — `crates/vox-doc-pipeline/src/pipeline/lint.rs:589`

**Substring**

```text
2026-05-03
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-03" "crates/vox-doc-pipeline/src/pipeline/lint.rs"`

**Confidence**: medium

---

### hv-1061 — `crates/vox-forge/src/github.rs:55`

**Substring**

```text
2022-11-28
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2022-11-28" "crates/vox-forge/src/github.rs"`

**Confidence**: medium

---

### hv-1062 — `crates/vox-forge/src/github.rs:100`

**Substring**

```text
2022-11-28
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2022-11-28" "crates/vox-forge/src/github.rs"`

**Confidence**: medium

---

### hv-1063 — `crates/vox-forge/src/types.rs:320`

**Substring**

```text
2026-01-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-01-01" "crates/vox-forge/src/types.rs"`

**Confidence**: medium

---

### hv-1064 — `crates/vox-forge/src/types.rs:321`

**Substring**

```text
2026-01-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-01-01" "crates/vox-forge/src/types.rs"`

**Confidence**: medium

---

### hv-1065 — `crates/vox-forge/src/types.rs:343`

**Substring**

```text
2026-01-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-01-01" "crates/vox-forge/src/types.rs"`

**Confidence**: medium

---

### hv-1066 — `crates/vox-forge/src/types.rs:344`

**Substring**

```text
2026-01-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-01-01" "crates/vox-forge/src/types.rs"`

**Confidence**: medium

---

### hv-1067 — `crates/vox-orchestrator-mcp/src/llm_bridge/providers/anthropic.rs:86`

**Substring**

```text
2023-06-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2023-06-01" "crates/vox-orchestrator-mcp/src/llm_bridge/providers/anthropic.rs"`

**Confidence**: medium

---

### hv-1068 — `crates/vox-orchestrator/src/catalog.rs:570`

**Substring**

```text
2023-06-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2023-06-01" "crates/vox-orchestrator/src/catalog.rs"`

**Confidence**: medium

---

### hv-1069 — `crates/vox-orchestrator/src/occ.rs:228`

**Substring**

```text
2026-03-22
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-22" "crates/vox-orchestrator/src/occ.rs"`

**Confidence**: medium

---

### hv-1070 — `crates/vox-orchestrator/src/occ.rs:229`

**Substring**

```text
2026-03-22
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-22" "crates/vox-orchestrator/src/occ.rs"`

**Confidence**: medium

---

### hv-1071 — `crates/vox-orchestrator/src/occ.rs:230`

**Substring**

```text
2026-03-22
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-22" "crates/vox-orchestrator/src/occ.rs"`

**Confidence**: medium

---

### hv-1072 — `crates/vox-orchestrator/src/occ.rs:250`

**Substring**

```text
2026-03-22
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-22" "crates/vox-orchestrator/src/occ.rs"`

**Confidence**: medium

---

### hv-1073 — `crates/vox-orchestrator/src/occ.rs:251`

**Substring**

```text
2026-03-22
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-22" "crates/vox-orchestrator/src/occ.rs"`

**Confidence**: medium

---

### hv-1074 — `crates/vox-orchestrator/src/occ.rs:273`

**Substring**

```text
2026-03-22
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-22" "crates/vox-orchestrator/src/occ.rs"`

**Confidence**: medium

---

### hv-1075 — `crates/vox-orchestrator/src/usage.rs:739`

**Substring**

```text
2026-03-02
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-03-02" "crates/vox-orchestrator/src/usage.rs"`

**Confidence**: medium

---

### hv-1076 — `crates/vox-populi/src/mens/discovery_publish.rs:113`

**Substring**

```text
2026-05-10
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-10" "crates/vox-populi/src/mens/discovery_publish.rs"`

**Confidence**: medium

---

### hv-1077 — `crates/vox-populi/src/mens/discovery_publish.rs:123`

**Substring**

```text
2026-05-10
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-10" "crates/vox-populi/src/mens/discovery_publish.rs"`

**Confidence**: medium

---

### hv-1078 — `crates/vox-populi/src/mens/discovery_publish.rs:136`

**Substring**

```text
2026-05-10
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-10" "crates/vox-populi/src/mens/discovery_publish.rs"`

**Confidence**: medium

---

### hv-1079 — `crates/vox-publisher/src/atlas/manifest.rs:97`

**Substring**

```text
2026-05-09
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-09" "crates/vox-publisher/src/atlas/manifest.rs"`

**Confidence**: medium

---

### hv-1080 — `crates/vox-publisher/src/atlas/manifest.rs:121`

**Substring**

```text
2026-05-09
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-09" "crates/vox-publisher/src/atlas/manifest.rs"`

**Confidence**: medium

---

### hv-1081 — `crates/vox-publisher/src/atlas/submission.rs:92`

**Substring**

```text
2026-05-09
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-09" "crates/vox-publisher/src/atlas/submission.rs"`

**Confidence**: medium

---

### hv-1082 — `crates/vox-publisher/src/atlas/submission.rs:143`

**Substring**

```text
2026-05-09
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-09" "crates/vox-publisher/src/atlas/submission.rs"`

**Confidence**: medium

---

### hv-1083 — `crates/vox-publisher/src/scholarly/crossref_deposit.rs:209`

**Substring**

```text
2026-05-09
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-05-09" "crates/vox-publisher/src/scholarly/crossref_deposit.rs"`

**Confidence**: medium

---

### hv-1084 — `crates/vox-publisher/src/switching.rs:549`

**Substring**

```text
2024-01-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2024-01-01" "crates/vox-publisher/src/switching.rs"`

**Confidence**: medium

---

### hv-1085 — `crates/vox-publisher/src/templates.rs:116`

**Substring**

```text
2026-01-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-01-01" "crates/vox-publisher/src/templates.rs"`

**Confidence**: medium

---

### hv-1086 — `crates/vox-ro-crate/src/metadata.rs:186`

**Substring**

```text
2023-11-15
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2023-11-15" "crates/vox-ro-crate/src/metadata.rs"`

**Confidence**: medium

---

### hv-1087 — `crates/vox-share/src/backends/cloudflare.rs:138`

**Substring**

```text
2024-01-01
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2024-01-01" "crates/vox-share/src/backends/cloudflare.rs"`

**Confidence**: medium

---

### hv-1088 — `examples/golden/iot_telemetry.vox:32`

**Substring**

```text
2026-04-13
```

**Why it matters**: Date constants in logic look like stale placeholders or wrong-era defaults.

**Fix** (review-intentional): If not a real release date / contract version, derive from build metadata or user input.

**Verify**: `rg -nF "2026-04-13" "examples/golden/iot_telemetry.vox"`

**Confidence**: medium

---

