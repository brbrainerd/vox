# 17 — hardcoded canonical keys

**Severity**: info  
**Itemized**: 100

### hv-1194 — `crates/vox-capability-registry/src/command_registry.rs:28`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-capability-registry/src/command_registry.rs"`

**Confidence**: medium

---

### hv-1195 — `crates/vox-cli/src/artifact_policy.rs:80`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/artifact_policy.rs"`

**Confidence**: medium

---

### hv-1196 — `crates/vox-cli/src/command_catalog.rs:425`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/command_catalog.rs"`

**Confidence**: medium

---

### hv-1197 — `crates/vox-cli/src/command_contract.rs:25`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/command_contract.rs"`

**Confidence**: medium

---

### hv-1198 — `crates/vox-cli/src/command_contract.rs:72`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/command_contract.rs"`

**Confidence**: medium

---

### hv-1199 — `crates/vox-cli/src/command_contract.rs:79`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/command_contract.rs"`

**Confidence**: medium

---

### hv-1200 — `crates/vox-cli/src/command_contract.rs:80`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/command_contract.rs"`

**Confidence**: medium

---

### hv-1201 — `crates/vox-cli/src/command_contract.rs:94`

**Substring**

```text
"populi"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"populi\"" "crates/vox-cli/src/command_contract.rs"`

**Confidence**: medium

---

### hv-1202 — `crates/vox-cli/src/command_contract.rs:95`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/command_contract.rs"`

**Confidence**: medium

---

### hv-1203 — `crates/vox-cli/src/command_contract.rs:171`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/command_contract.rs"`

**Confidence**: medium

---

### hv-1204 — `crates/vox-cli/src/command_contract.rs:172`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/command_contract.rs"`

**Confidence**: medium

---

### hv-1205 — `crates/vox-cli/src/commands/ci/capability_sync.rs:34`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/capability_sync.rs"`

**Confidence**: medium

---

### hv-1206 — `crates/vox-cli/src/commands/ci/command_compliance/capability_registry.rs:29`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/command_compliance/capability_registry.rs"`

**Confidence**: medium

---

### hv-1207 — `crates/vox-cli/src/commands/ci/command_compliance/validators.rs:28`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/commands/ci/command_compliance/validators.rs"`

**Confidence**: medium

---

### hv-1208 — `crates/vox-cli/src/commands/ci/command_compliance/validators.rs:56`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/command_compliance/validators.rs"`

**Confidence**: medium

---

### hv-1209 — `crates/vox-cli/src/commands/ci/command_compliance/validators.rs:508`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/command_compliance/validators.rs"`

**Confidence**: medium

---

### hv-1210 — `crates/vox-cli/src/commands/ci/command_compliance/validators.rs:557`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/command_compliance/validators.rs"`

**Confidence**: medium

---

### hv-1211 — `crates/vox-cli/src/commands/ci/command_compliance/validators.rs:610`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/command_compliance/validators.rs"`

**Confidence**: medium

---

### hv-1212 — `crates/vox-cli/src/commands/ci/command_compliance/validators.rs:622`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/commands/ci/command_compliance/validators.rs"`

**Confidence**: medium

---

### hv-1213 — `crates/vox-cli/src/commands/ci/command_compliance/validators.rs:718`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/command_compliance/validators.rs"`

**Confidence**: medium

---

### hv-1214 — `crates/vox-cli/src/commands/ci/command_compliance/validators.rs:1113`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/commands/ci/command_compliance/validators.rs"`

**Confidence**: medium

---

### hv-1215 — `crates/vox-cli/src/commands/ci/command_sync.rs:49`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/command_sync.rs"`

**Confidence**: medium

---

### hv-1216 — `crates/vox-cli/src/commands/ci/command_sync.rs:57`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/command_sync.rs"`

**Confidence**: medium

---

### hv-1217 — `crates/vox-cli/src/commands/ci/compile_matrix.rs:60`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/compile_matrix.rs"`

**Confidence**: medium

---

### hv-1218 — `crates/vox-cli/src/commands/ci/dep_sprawl.rs:32`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/dep_sprawl.rs"`

**Confidence**: medium

---

### hv-1219 — `crates/vox-cli/src/commands/ci/dep_sprawl.rs:36`

**Substring**

```text
"vox-orchestrator"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-orchestrator\"" "crates/vox-cli/src/commands/ci/dep_sprawl.rs"`

**Confidence**: medium

---

### hv-1220 — `crates/vox-cli/src/commands/ci/deploy_status.rs:22`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/deploy_status.rs"`

**Confidence**: medium

---

### hv-1221 — `crates/vox-cli/src/commands/ci/deploy_status.rs:64`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/deploy_status.rs"`

**Confidence**: medium

---

### hv-1222 — `crates/vox-cli/src/commands/ci/determinism_audit.rs:47`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/determinism_audit.rs"`

**Confidence**: medium

---

### hv-1223 — `crates/vox-cli/src/commands/ci/determinism_audit.rs:66`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/determinism_audit.rs"`

**Confidence**: medium

---

### hv-1224 — `crates/vox-cli/src/commands/ci/mens_scorecard.rs:452`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/commands/ci/mens_scorecard.rs"`

**Confidence**: medium

---

### hv-1225 — `crates/vox-cli/src/commands/ci/nomenclature_guard.rs:30`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/commands/ci/nomenclature_guard.rs"`

**Confidence**: medium

---

### hv-1226 — `crates/vox-cli/src/commands/ci/nomenclature_guard.rs:31`

**Substring**

```text
"populi"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"populi\"" "crates/vox-cli/src/commands/ci/nomenclature_guard.rs"`

**Confidence**: medium

---

### hv-1227 — `crates/vox-cli/src/commands/ci/nomenclature_guard.rs:34`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/commands/ci/nomenclature_guard.rs"`

**Confidence**: medium

---

### hv-1228 — `crates/vox-cli/src/commands/ci/nomenclature_guard.rs:43`

**Substring**

```text
"vox-orchestrator"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-orchestrator\"" "crates/vox-cli/src/commands/ci/nomenclature_guard.rs"`

**Confidence**: medium

---

### hv-1229 — `crates/vox-cli/src/commands/ci/operations_catalog.rs:332`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/operations_catalog.rs"`

**Confidence**: medium

---

### hv-1230 — `crates/vox-cli/src/commands/ci/operations_catalog.rs:461`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/operations_catalog.rs"`

**Confidence**: medium

---

### hv-1231 — `crates/vox-cli/src/commands/ci/operations_catalog.rs:740`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/operations_catalog.rs"`

**Confidence**: medium

---

### hv-1232 — `crates/vox-cli/src/commands/ci/operations_catalog.rs:756`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/operations_catalog.rs"`

**Confidence**: medium

---

### hv-1233 — `crates/vox-cli/src/commands/ci/pm_provenance.rs:123`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/pm_provenance.rs"`

**Confidence**: medium

---

### hv-1234 — `crates/vox-cli/src/commands/ci/pre_push.rs:523`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/pre_push.rs"`

**Confidence**: medium

---

### hv-1235 — `crates/vox-cli/src/commands/ci/pre_push.rs:530`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/pre_push.rs"`

**Confidence**: medium

---

### hv-1236 — `crates/vox-cli/src/commands/ci/pre_push.rs:537`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/pre_push.rs"`

**Confidence**: medium

---

### hv-1237 — `crates/vox-cli/src/commands/ci/pre_push.rs:548`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/pre_push.rs"`

**Confidence**: medium

---

### hv-1238 — `crates/vox-cli/src/commands/ci/pre_push.rs:563`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/pre_push.rs"`

**Confidence**: medium

---

### hv-1239 — `crates/vox-cli/src/commands/ci/pre_push.rs:578`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/pre_push.rs"`

**Confidence**: medium

---

### hv-1240 — `crates/vox-cli/src/commands/ci/pre_push.rs:613`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/pre_push.rs"`

**Confidence**: medium

---

### hv-1241 — `crates/vox-cli/src/commands/ci/pre_push.rs:665`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/pre_push.rs"`

**Confidence**: medium

---

### hv-1242 — `crates/vox-cli/src/commands/ci/pre_push.rs:684`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/pre_push.rs"`

**Confidence**: medium

---

### hv-1243 — `crates/vox-cli/src/commands/ci/release_build.rs:72`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/release_build.rs"`

**Confidence**: medium

---

### hv-1244 — `crates/vox-cli/src/commands/ci/run_body_helpers/cuda_release_build.rs:29`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/run_body_helpers/cuda_release_build.rs"`

**Confidence**: medium

---

### hv-1245 — `crates/vox-cli/src/commands/ci/run_body_helpers/cuda.rs:32`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/run_body_helpers/cuda.rs"`

**Confidence**: medium

---

### hv-1246 — `crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs:248`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs"`

**Confidence**: medium

---

### hv-1247 — `crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs:354`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs"`

**Confidence**: medium

---

### hv-1248 — `crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs:560`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs"`

**Confidence**: medium

---

### hv-1249 — `crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs:570`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs"`

**Confidence**: medium

---

### hv-1250 — `crates/vox-cli/src/commands/ci/run_body_helpers/matrix/tests.rs:24`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/commands/ci/run_body_helpers/matrix/tests.rs"`

**Confidence**: medium

---

### hv-1251 — `crates/vox-cli/src/commands/ci/run_body_helpers/timings.rs:117`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/run_body_helpers/timings.rs"`

**Confidence**: medium

---

### hv-1252 — `crates/vox-cli/src/commands/ci/run_body_helpers/timings.rs:132`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/run_body_helpers/timings.rs"`

**Confidence**: medium

---

### hv-1253 — `crates/vox-cli/src/commands/ci/run_body_helpers/timings.rs:142`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/commands/ci/run_body_helpers/timings.rs"`

**Confidence**: medium

---

### hv-1254 — `crates/vox-cli/src/commands/ci/run_body.rs:348`

**Substring**

```text
"vox-orchestrator"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-orchestrator\"" "crates/vox-cli/src/commands/ci/run_body.rs"`

**Confidence**: medium

---

### hv-1255 — `crates/vox-cli/src/commands/ci/run_body.rs:382`

**Substring**

```text
"vox-orchestrator"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-orchestrator\"" "crates/vox-cli/src/commands/ci/run_body.rs"`

**Confidence**: medium

---

### hv-1256 — `crates/vox-cli/src/commands/ci/speech_runtime_suite.rs:146`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/commands/ci/speech_runtime_suite.rs"`

**Confidence**: medium

---

### hv-1257 — `crates/vox-cli/src/commands/ci/watch_run.rs:227`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/watch_run.rs"`

**Confidence**: medium

---

### hv-1258 — `crates/vox-cli/src/commands/ci/watch_run.rs:256`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/ci/watch_run.rs"`

**Confidence**: medium

---

### hv-1259 — `crates/vox-cli/src/commands/ci/workspace_artifacts/mod.rs:203`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/commands/ci/workspace_artifacts/mod.rs"`

**Confidence**: medium

---

### hv-1260 — `crates/vox-cli/src/commands/ci/workspace_artifacts/mod.rs:446`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/commands/ci/workspace_artifacts/mod.rs"`

**Confidence**: medium

---

### hv-1261 — `crates/vox-cli/src/commands/ci/workspace_artifacts/retention.rs:156`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/commands/ci/workspace_artifacts/retention.rs"`

**Confidence**: medium

---

### hv-1262 — `crates/vox-cli/src/commands/diagnostics/doctor/checks_codex.rs:50`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/diagnostics/doctor/checks_codex.rs"`

**Confidence**: medium

---

### hv-1263 — `crates/vox-cli/src/commands/review/coderabbit/config.rs:134`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/commands/review/coderabbit/config.rs"`

**Confidence**: medium

---

### hv-1264 — `crates/vox-cli/src/commands/review/coderabbit/ingest.rs:129`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-cli/src/commands/review/coderabbit/ingest.rs"`

**Confidence**: medium

---

### hv-1265 — `crates/vox-cli/src/lib.rs:562`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/lib.rs"`

**Confidence**: medium

---

### hv-1266 — `crates/vox-cli/src/main.rs:60`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/main.rs"`

**Confidence**: medium

---

### hv-1267 — `crates/vox-cli/src/main.rs:66`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-cli/src/main.rs"`

**Confidence**: medium

---

### hv-1268 — `crates/vox-cli/src/main.rs:80`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-cli/src/main.rs"`

**Confidence**: medium

---

### hv-1269 — `crates/vox-code-audit/src/detectors/adr_citation.rs:36`

**Substring**

```text
"vox-orchestrator"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-orchestrator\"" "crates/vox-code-audit/src/detectors/adr_citation.rs"`

**Confidence**: medium

---

### hv-1270 — `crates/vox-compiler/src/hir/lower/mod.rs:857`

**Substring**

```text
"populi"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"populi\"" "crates/vox-compiler/src/hir/lower/mod.rs"`

**Confidence**: medium

---

### hv-1271 — `crates/vox-compiler/src/hir/lower/mod.rs:870`

**Substring**

```text
"populi"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"populi\"" "crates/vox-compiler/src/hir/lower/mod.rs"`

**Confidence**: medium

---

### hv-1272 — `crates/vox-compiler/src/typeck/checker/expr_ops.rs:119`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-compiler/src/typeck/checker/expr_ops.rs"`

**Confidence**: medium

---

### hv-1273 — `crates/vox-corpus/build.rs:399`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-corpus/build.rs"`

**Confidence**: medium

---

### hv-1274 — `crates/vox-corpus/src/synthetic_gen/bodies/_tool_pairs_body.rs:77`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-corpus/src/synthetic_gen/bodies/_tool_pairs_body.rs"`

**Confidence**: medium

---

### hv-1275 — `crates/vox-corpus/src/synthetic_gen/bodies/_tool_pairs_body.rs:79`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-corpus/src/synthetic_gen/bodies/_tool_pairs_body.rs"`

**Confidence**: medium

---

### hv-1276 — `crates/vox-drift-check/src/rules/timeout_literal.rs:73`

**Substring**

```text
"vox-orchestrator"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-orchestrator\"" "crates/vox-drift-check/src/rules/timeout_literal.rs"`

**Confidence**: medium

---

### hv-1277 — `crates/vox-drift-check/src/rules/vox_path_literal.rs:77`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-drift-check/src/rules/vox_path_literal.rs"`

**Confidence**: medium

---

### hv-1278 — `crates/vox-exec-grammar/src/ast.rs:393`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-exec-grammar/src/ast.rs"`

**Confidence**: medium

---

### hv-1279 — `crates/vox-gamify/src/quest/slots.rs:7`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-gamify/src/quest/slots.rs"`

**Confidence**: medium

---

### hv-1280 — `crates/vox-gamify/src/quest/slots.rs:9`

**Substring**

```text
"vox-orchestrator"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-orchestrator\"" "crates/vox-gamify/src/quest/slots.rs"`

**Confidence**: medium

---

### hv-1281 — `crates/vox-gamify/src/quest/slots.rs:44`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-gamify/src/quest/slots.rs"`

**Confidence**: medium

---

### hv-1282 — `crates/vox-ml-cli/src/commands/ai/generate.rs:40`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-ml-cli/src/commands/ai/generate.rs"`

**Confidence**: medium

---

### hv-1283 — `crates/vox-ml-cli/src/commands/ai/train.rs:71`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-ml-cli/src/commands/ai/train.rs"`

**Confidence**: medium

---

### hv-1284 — `crates/vox-ml-cli/src/commands/corpus/stats.rs:184`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-ml-cli/src/commands/corpus/stats.rs"`

**Confidence**: medium

---

### hv-1285 — `crates/vox-ml-cli/src/commands/mens/populi/action_populi_enum.rs:5`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-ml-cli/src/commands/mens/populi/action_populi_enum.rs"`

**Confidence**: medium

---

### hv-1286 — `crates/vox-ml-cli/src/commands/oratio_cmd.rs:27`

**Substring**

```text
"oratio"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"oratio\"" "crates/vox-ml-cli/src/commands/oratio_cmd.rs"`

**Confidence**: medium

---

### hv-1287 — `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:40`

**Substring**

```text
"populi"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"populi\"" "crates/vox-ml-cli/src/commands/populi_lifecycle.rs"`

**Confidence**: medium

---

### hv-1288 — `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:180`

**Substring**

```text
"populi"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"populi\"" "crates/vox-ml-cli/src/commands/populi_lifecycle.rs"`

**Confidence**: medium

---

### hv-1289 — `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:220`

**Substring**

```text
"populi"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"populi\"" "crates/vox-ml-cli/src/commands/populi_lifecycle.rs"`

**Confidence**: medium

---

### hv-1290 — `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:235`

**Substring**

```text
"populi"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"populi\"" "crates/vox-ml-cli/src/commands/populi_lifecycle.rs"`

**Confidence**: medium

---

### hv-1291 — `crates/vox-openclaw-runtime/src/openclaw_gateway_ws.rs:42`

**Substring**

```text
"vox-cli"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"vox-cli\"" "crates/vox-openclaw-runtime/src/openclaw_gateway_ws.rs"`

**Confidence**: medium

---

### hv-1292 — `crates/vox-oratio/src/contextual_bias.rs:73`

**Substring**

```text
"MENS"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"MENS\"" "crates/vox-oratio/src/contextual_bias.rs"`

**Confidence**: medium

---

### hv-1293 — `crates/vox-oratio/src/refine/rules.rs:15`

**Substring**

```text
"mens"
```

**Why it matters**: Duplicated capability/plugin keys drift from the registry SSOT.

**Fix** (centralize-in-contract): Import plugin / capability id from registry SSOT instead of duplicating string literals.

**SSOT**: `contracts/capability/capability-registry.yaml`

**Verify**: `rg -nF "\"mens\"" "crates/vox-oratio/src/refine/rules.rs"`

**Confidence**: medium

---

