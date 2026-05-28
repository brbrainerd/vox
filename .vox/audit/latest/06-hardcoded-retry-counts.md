# 06 — hardcoded retry counts

**Severity**: warning  
**Itemized**: 43

### hv-0382 — `contracts/reports/scaling-audit/findings-latest.json:25763`

**Substring**

```text
for _ in 0..100
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..100" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0383 — `contracts/reports/scaling-audit/findings-latest.json:36851`

**Substring**

```text
for i in 0..5
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for i in 0..5" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0384 — `crates/vox-code-audit/src/detectors/long_range_coupling.rs:204`

**Substring**

```text
for _ in 0..100
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..100" "crates/vox-code-audit/src/detectors/long_range_coupling.rs"`

**Confidence**: medium

---

### hv-0385 — `crates/vox-code-audit/src/detectors/long_range_coupling.rs:222`

**Substring**

```text
for _ in 0..5
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..5" "crates/vox-code-audit/src/detectors/long_range_coupling.rs"`

**Confidence**: medium

---

### hv-0386 — `crates/vox-code-audit/src/detectors/mod.rs:213`

**Substring**

```text
for _ in 0..600
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..600" "crates/vox-code-audit/src/detectors/mod.rs"`

**Confidence**: medium

---

### hv-0387 — `crates/vox-constrained-gen/src/deadlock.rs:33`

**Substring**

```text
max_retries: 3
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "max_retries: 3" "crates/vox-constrained-gen/src/deadlock.rs"`

**Confidence**: medium

---

### hv-0388 — `crates/vox-corpus/src/codegen_vox/part_03.rs:61`

**Substring**

```text
for v in 0..5
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for v in 0..5" "crates/vox-corpus/src/codegen_vox/part_03.rs"`

**Confidence**: medium

---

### hv-0389 — `crates/vox-corpus/src/corpus/augment/tests_mod.rs:83`

**Substring**

```text
for _ in 0..100
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..100" "crates/vox-corpus/src/corpus/augment/tests_mod.rs"`

**Confidence**: medium

---

### hv-0390 — `crates/vox-db/src/circuit_breaker.rs:176`

**Substring**

```text
for _ in 0..3
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..3" "crates/vox-db/src/circuit_breaker.rs"`

**Confidence**: medium

---

### hv-0391 — `crates/vox-db/src/circuit_breaker.rs:217`

**Substring**

```text
for _ in 0..10
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..10" "crates/vox-db/src/circuit_breaker.rs"`

**Confidence**: medium

---

### hv-0392 — `crates/vox-db/src/store/ops_retention.rs:128`

**Substring**

```text
for _ in 0..10
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..10" "crates/vox-db/src/store/ops_retention.rs"`

**Confidence**: medium

---

### hv-0393 — `crates/vox-gamify/src/achievement/tracker.rs:144`

**Substring**

```text
for _ in 0..4
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..4" "crates/vox-gamify/src/achievement/tracker.rs"`

**Confidence**: medium

---

### hv-0394 — `crates/vox-gamify/src/reward_policy.rs:619`

**Substring**

```text
for _ in 0..10
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..10" "crates/vox-gamify/src/reward_policy.rs"`

**Confidence**: medium

---

### hv-0395 — `crates/vox-gamify/src/teaching.rs:387`

**Substring**

```text
for _ in 0..3
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..3" "crates/vox-gamify/src/teaching.rs"`

**Confidence**: medium

---

### hv-0396 — `crates/vox-gamify/src/teaching.rs:400`

**Substring**

```text
for _ in 0..3
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..3" "crates/vox-gamify/src/teaching.rs"`

**Confidence**: medium

---

### hv-0397 — `crates/vox-gamify/src/teaching.rs:412`

**Substring**

```text
for _ in 0..5
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..5" "crates/vox-gamify/src/teaching.rs"`

**Confidence**: medium

---

### hv-0398 — `crates/vox-orchestrator-mcp/src/compiler_tools.rs:467`

**Substring**

```text
retry_count = 0
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "retry_count = 0" "crates/vox-orchestrator-mcp/src/compiler_tools.rs"`

**Confidence**: medium

---

### hv-0399 — `crates/vox-orchestrator-mcp/src/compiler_tools.rs:936`

**Substring**

```text
max_retries = 3
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "max_retries = 3" "crates/vox-orchestrator-mcp/src/compiler_tools.rs"`

**Confidence**: medium

---

### hv-0400 — `crates/vox-orchestrator-mcp/src/compiler_tools.rs:937`

**Substring**

```text
retry_count = 0
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "retry_count = 0" "crates/vox-orchestrator-mcp/src/compiler_tools.rs"`

**Confidence**: medium

---

### hv-0401 — `crates/vox-orchestrator-mcp/src/llm_bridge/provider_adapter.rs:278`

**Substring**

```text
max_retries: 3
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "max_retries: 3" "crates/vox-orchestrator-mcp/src/llm_bridge/provider_adapter.rs"`

**Confidence**: medium

---

### hv-0402 — `crates/vox-orchestrator-queue/src/oplog/mod.rs:271`

**Substring**

```text
for i in 0..5
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for i in 0..5" "crates/vox-orchestrator-queue/src/oplog/mod.rs"`

**Confidence**: medium

---

### hv-0403 — `crates/vox-orchestrator/src/attention/mod.rs:75`

**Substring**

```text
for _ in 0..5
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..5" "crates/vox-orchestrator/src/attention/mod.rs"`

**Confidence**: medium

---

### hv-0404 — `crates/vox-orchestrator/src/calibration.rs:303`

**Substring**

```text
for _ in 0..4
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..4" "crates/vox-orchestrator/src/calibration.rs"`

**Confidence**: medium

---

### hv-0405 — `crates/vox-orchestrator/src/calibration.rs:341`

**Substring**

```text
for _ in 0..100
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..100" "crates/vox-orchestrator/src/calibration.rs"`

**Confidence**: medium

---

### hv-0406 — `crates/vox-orchestrator/src/observer.rs:447`

**Substring**

```text
for _ in 0..25
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..25" "crates/vox-orchestrator/src/observer.rs"`

**Confidence**: medium

---

### hv-0407 — `crates/vox-orchestrator/src/orchestrator/persistence/lifecycle.rs:39`

**Substring**

```text
max_retries: 128
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "max_retries: 128" "crates/vox-orchestrator/src/orchestrator/persistence/lifecycle.rs"`

**Confidence**: medium

---

### hv-0408 — `crates/vox-orchestrator/src/orchestrator/persistence/lifecycle.rs:247`

**Substring**

```text
max_retries: 10
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "max_retries: 10" "crates/vox-orchestrator/src/orchestrator/persistence/lifecycle.rs"`

**Confidence**: medium

---

### hv-0409 — `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/goal.rs:28`

**Substring**

```text
for _ in 0..24
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..24" "crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/goal.rs"`

**Confidence**: medium

---

### hv-0410 — `crates/vox-orchestrator/src/planning/synthesizer.rs:357`

**Substring**

```text
for i in 0..50
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for i in 0..50" "crates/vox-orchestrator/src/planning/synthesizer.rs"`

**Confidence**: medium

---

### hv-0411 — `crates/vox-orchestrator/src/planning/types.rs:88`

**Substring**

```text
max_retries: 1
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "max_retries: 1" "crates/vox-orchestrator/src/planning/types.rs"`

**Confidence**: medium

---

### hv-0412 — `crates/vox-orchestrator/src/queue/mod.rs:215`

**Substring**

```text
retry_count = 3
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "retry_count = 3" "crates/vox-orchestrator/src/queue/mod.rs"`

**Confidence**: medium

---

### hv-0413 — `crates/vox-orchestrator/src/rebalance.rs:193`

**Substring**

```text
for i in 0..15
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for i in 0..15" "crates/vox-orchestrator/src/rebalance.rs"`

**Confidence**: medium

---

### hv-0414 — `crates/vox-orchestrator/src/routing/bandit.rs:28`

**Substring**

```text
for _ in 0..100
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..100" "crates/vox-orchestrator/src/routing/bandit.rs"`

**Confidence**: medium

---

### hv-0415 — `crates/vox-orchestrator/src/security.rs:400`

**Substring**

```text
for _ in 0..5
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..5" "crates/vox-orchestrator/src/security.rs"`

**Confidence**: medium

---

### hv-0416 — `crates/vox-orchestrator/src/security.rs:433`

**Substring**

```text
for i in 0..5
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for i in 0..5" "crates/vox-orchestrator/src/security.rs"`

**Confidence**: medium

---

### hv-0417 — `crates/vox-orchestrator/src/types/tasks.rs:483`

**Substring**

```text
retry_count: 0
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "retry_count: 0" "crates/vox-orchestrator/src/types/tasks.rs"`

**Confidence**: medium

---

### hv-0418 — `crates/vox-scaling-policy/src/cost_defense.rs:302`

**Substring**

```text
for _ in 0..3
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..3" "crates/vox-scaling-policy/src/cost_defense.rs"`

**Confidence**: medium

---

### hv-0419 — `crates/vox-secrets/src/tests.rs:278`

**Substring**

```text
for _ in 0..8
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for _ in 0..8" "crates/vox-secrets/src/tests.rs"`

**Confidence**: medium

---

### hv-0420 — `crates/vox-tensor/src/data.rs:600`

**Substring**

```text
for i in 0..7
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for i in 0..7" "crates/vox-tensor/src/data.rs"`

**Confidence**: medium

---

### hv-0421 — `crates/vox-tensor/src/replay.rs:292`

**Substring**

```text
for i in 0..5
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for i in 0..5" "crates/vox-tensor/src/replay.rs"`

**Confidence**: medium

---

### hv-0422 — `crates/vox-tensor/src/replay.rs:314`

**Substring**

```text
for i in 0..10
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for i in 0..10" "crates/vox-tensor/src/replay.rs"`

**Confidence**: medium

---

### hv-0423 — `crates/vox-tensor/src/replay.rs:363`

**Substring**

```text
for i in 0..5
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "for i in 0..5" "crates/vox-tensor/src/replay.rs"`

**Confidence**: medium

---

### hv-0424 — `crates/vox-webhook/src/delivery.rs:28`

**Substring**

```text
max_retries: 3
```

**Why it matters**: Implicit retry limits cause flaky recovery or excessive load.

**Fix** (extract-named-constant): const MAX_RETRIES: u32 = …; // tune via config when needed

**Verify**: `rg -nF "max_retries: 3" "crates/vox-webhook/src/delivery.rs"`

**Confidence**: medium

---

