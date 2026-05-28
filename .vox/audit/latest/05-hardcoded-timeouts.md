# 05 — hardcoded timeouts

**Severity**: warning  
**Itemized**: 100

### hv-0282 — `apps/editor/vox-vscode/src/VisualEditorPanel.ts:197`

**Substring**

```text
setTimeout(findServer, 5000)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "setTimeout(findServer, 5000)" "apps/editor/vox-vscode/src/VisualEditorPanel.ts"`

**Confidence**: high

---

### hv-0283 — `contracts/reports/scaling-audit/findings-latest.json:20177`

**Substring**

```text
Duration::from_millis(250)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(250)" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: high

---

### hv-0284 — `contracts/reports/scaling-audit/findings-scaling-latest.json:1584`

**Substring**

```text
Duration::from_millis(250)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(250)" "contracts/reports/scaling-audit/findings-scaling-latest.json"`

**Confidence**: high

---

### hv-0285 — `crates/vox-actor-runtime/src/activity.rs:29`

**Substring**

```text
Duration::from_millis(100)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(100)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0286 — `crates/vox-actor-runtime/src/activity.rs:30`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0287 — `crates/vox-actor-runtime/src/activity.rs:306`

**Substring**

```text
Duration::from_millis(100)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(100)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0288 — `crates/vox-actor-runtime/src/activity.rs:307`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0289 — `crates/vox-actor-runtime/src/activity.rs:317`

**Substring**

```text
Duration::from_millis(200)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(200)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0290 — `crates/vox-actor-runtime/src/activity.rs:321`

**Substring**

```text
Duration::from_secs(10)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(10)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0291 — `crates/vox-actor-runtime/src/activity.rs:322`

**Substring**

```text
Duration::from_millis(200)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(200)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0292 — `crates/vox-actor-runtime/src/activity.rs:330`

**Substring**

```text
Duration::from_secs(10)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(10)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0293 — `crates/vox-actor-runtime/src/activity.rs:334`

**Substring**

```text
Duration::from_millis(500)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(500)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0294 — `crates/vox-actor-runtime/src/activity.rs:338`

**Substring**

```text
Duration::from_secs(120)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(120)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0295 — `crates/vox-actor-runtime/src/activity.rs:342`

**Substring**

```text
Duration::from_secs(3600)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3600)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0296 — `crates/vox-actor-runtime/src/activity.rs:346`

**Substring**

```text
Duration::from_secs(30)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(30)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0297 — `crates/vox-actor-runtime/src/activity.rs:353`

**Substring**

```text
Duration::from_secs(5)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(5)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0298 — `crates/vox-actor-runtime/src/activity.rs:356`

**Substring**

```text
Duration::from_secs(4)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(4)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0299 — `crates/vox-actor-runtime/src/activity.rs:360`

**Substring**

```text
Duration::from_secs(5)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(5)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0300 — `crates/vox-actor-runtime/src/activity.rs:379`

**Substring**

```text
Duration::from_millis(1)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(1)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0301 — `crates/vox-actor-runtime/src/activity.rs:406`

**Substring**

```text
Duration::from_millis(1)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(1)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0302 — `crates/vox-actor-runtime/src/activity.rs:427`

**Substring**

```text
Duration::from_millis(10)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(10)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0303 — `crates/vox-actor-runtime/src/activity.rs:430`

**Substring**

```text
Duration::from_secs(10)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(10)" "crates/vox-actor-runtime/src/activity.rs"`

**Confidence**: high

---

### hv-0304 — `crates/vox-actor-runtime/src/durable_scheduler.rs:83`

**Substring**

```text
Duration::from_secs(3600)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3600)" "crates/vox-actor-runtime/src/durable_scheduler.rs"`

**Confidence**: high

---

### hv-0305 — `crates/vox-actor-runtime/src/durable_scheduler.rs:86`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-actor-runtime/src/durable_scheduler.rs"`

**Confidence**: high

---

### hv-0306 — `crates/vox-actor-runtime/src/durable_scheduler.rs:185`

**Substring**

```text
Duration::from_secs(10)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(10)" "crates/vox-actor-runtime/src/durable_scheduler.rs"`

**Confidence**: high

---

### hv-0307 — `crates/vox-actor-runtime/src/durable_scheduler.rs:187`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-actor-runtime/src/durable_scheduler.rs"`

**Confidence**: high

---

### hv-0308 — `crates/vox-actor-runtime/src/durable_scheduler.rs:195`

**Substring**

```text
Duration::from_secs(7200)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(7200)" "crates/vox-actor-runtime/src/durable_scheduler.rs"`

**Confidence**: high

---

### hv-0309 — `crates/vox-actor-runtime/src/durable_scheduler.rs:198`

**Substring**

```text
Duration::from_secs(3600)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3600)" "crates/vox-actor-runtime/src/durable_scheduler.rs"`

**Confidence**: high

---

### hv-0310 — `crates/vox-actor-runtime/src/durable_scheduler.rs:208`

**Substring**

```text
Duration::from_secs(3600)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3600)" "crates/vox-actor-runtime/src/durable_scheduler.rs"`

**Confidence**: high

---

### hv-0311 — `crates/vox-actor-runtime/src/inference_env.rs:90`

**Substring**

```text
Duration::from_secs(30)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(30)" "crates/vox-actor-runtime/src/inference_env.rs"`

**Confidence**: high

---

### hv-0312 — `crates/vox-actor-runtime/src/inference_env.rs:129`

**Substring**

```text
Duration::from_secs(5)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(5)" "crates/vox-actor-runtime/src/inference_env.rs"`

**Confidence**: high

---

### hv-0313 — `crates/vox-actor-runtime/src/presence.rs:120`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-actor-runtime/src/presence.rs"`

**Confidence**: high

---

### hv-0314 — `crates/vox-actor-runtime/src/presence.rs:131`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-actor-runtime/src/presence.rs"`

**Confidence**: high

---

### hv-0315 — `crates/vox-actor-runtime/src/presence.rs:141`

**Substring**

```text
Duration::from_millis(1)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(1)" "crates/vox-actor-runtime/src/presence.rs"`

**Confidence**: high

---

### hv-0316 — `crates/vox-actor-runtime/src/presence.rs:142`

**Substring**

```text
Duration::from_secs(10)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(10)" "crates/vox-actor-runtime/src/presence.rs"`

**Confidence**: high

---

### hv-0317 — `crates/vox-actor-runtime/src/presence.rs:154`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-actor-runtime/src/presence.rs"`

**Confidence**: high

---

### hv-0318 — `crates/vox-actor-runtime/src/rate_limit.rs:54`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-actor-runtime/src/rate_limit.rs"`

**Confidence**: high

---

### hv-0319 — `crates/vox-actor-runtime/src/resilient_http.rs:130`

**Substring**

```text
Duration::from_millis(50)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(50)" "crates/vox-actor-runtime/src/resilient_http.rs"`

**Confidence**: high

---

### hv-0320 — `crates/vox-actor-runtime/src/resilient_http.rs:131`

**Substring**

```text
Duration::from_millis(100)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(100)" "crates/vox-actor-runtime/src/resilient_http.rs"`

**Confidence**: high

---

### hv-0321 — `crates/vox-actor-runtime/src/resilient_http.rs:132`

**Substring**

```text
Duration::from_millis(200)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(200)" "crates/vox-actor-runtime/src/resilient_http.rs"`

**Confidence**: high

---

### hv-0322 — `crates/vox-actor-runtime/src/subscription.rs:146`

**Substring**

```text
Duration::from_millis(100)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(100)" "crates/vox-actor-runtime/src/subscription.rs"`

**Confidence**: high

---

### hv-0323 — `crates/vox-actor-runtime/src/subscription.rs:158`

**Substring**

```text
Duration::from_millis(50)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(50)" "crates/vox-actor-runtime/src/subscription.rs"`

**Confidence**: high

---

### hv-0324 — `crates/vox-actor-runtime/src/supervisor.rs:158`

**Substring**

```text
Duration::from_millis(100)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(100)" "crates/vox-actor-runtime/src/supervisor.rs"`

**Confidence**: high

---

### hv-0325 — `crates/vox-claim-extractor/src/minicheck.rs:84`

**Substring**

```text
Duration::from_secs(10)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(10)" "crates/vox-claim-extractor/src/minicheck.rs"`

**Confidence**: high

---

### hv-0326 — `crates/vox-cli/src/commands/ci/pre_push.rs:174`

**Substring**

```text
Duration::from_secs(3)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3)" "crates/vox-cli/src/commands/ci/pre_push.rs"`

**Confidence**: high

---

### hv-0327 — `crates/vox-cli/src/commands/ci/test_inventory.rs:1115`

**Substring**

```text
Duration::from_millis(1)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(1)" "crates/vox-cli/src/commands/ci/test_inventory.rs"`

**Confidence**: high

---

### hv-0328 — `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs:213`

**Substring**

```text
Duration::from_millis(300)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(300)" "crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs"`

**Confidence**: high

---

### hv-0329 — `crates/vox-cli/src/commands/extras/ludus/hud.rs:22`

**Substring**

```text
Duration::from_secs(1)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(1)" "crates/vox-cli/src/commands/extras/ludus/hud.rs"`

**Confidence**: high

---

### hv-0330 — `crates/vox-cli/src/commands/extras/ludus/hud.rs:80`

**Substring**

```text
Duration::from_secs(3)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3)" "crates/vox-cli/src/commands/extras/ludus/hud.rs"`

**Confidence**: high

---

### hv-0331 — `crates/vox-cli/src/commands/generate.rs:30`

**Substring**

```text
Duration::from_secs(120)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(120)" "crates/vox-cli/src/commands/generate.rs"`

**Confidence**: high

---

### hv-0332 — `crates/vox-cli/src/commands/live.rs:235`

**Substring**

```text
Duration::from_millis(250)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(250)" "crates/vox-cli/src/commands/live.rs"`

**Confidence**: high

---

### hv-0333 — `crates/vox-cli/src/commands/live.rs:298`

**Substring**

```text
Duration::from_millis(250)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(250)" "crates/vox-cli/src/commands/live.rs"`

**Confidence**: high

---

### hv-0334 — `crates/vox-cli/src/commands/openclaw.rs:552`

**Substring**

```text
Duration::from_secs(5)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(5)" "crates/vox-cli/src/commands/openclaw.rs"`

**Confidence**: high

---

### hv-0335 — `crates/vox-cli/src/commands/openclaw.rs:596`

**Substring**

```text
Duration::from_secs(5)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(5)" "crates/vox-cli/src/commands/openclaw.rs"`

**Confidence**: high

---

### hv-0336 — `crates/vox-cli/src/commands/openclaw.rs:795`

**Substring**

```text
Duration::from_secs(5)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(5)" "crates/vox-cli/src/commands/openclaw.rs"`

**Confidence**: high

---

### hv-0337 — `crates/vox-cli/src/commands/plugin/publish.rs:143`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-cli/src/commands/plugin/publish.rs"`

**Confidence**: high

---

### hv-0338 — `crates/vox-cli/src/commands/repair.rs:47`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-cli/src/commands/repair.rs"`

**Confidence**: high

---

### hv-0339 — `crates/vox-cli/src/commands/research/mod.rs:261`

**Substring**

```text
Duration::from_secs(3)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3)" "crates/vox-cli/src/commands/research/mod.rs"`

**Confidence**: high

---

### hv-0340 — `crates/vox-cli/src/commands/review/coderabbit/github/comments.rs:52`

**Substring**

```text
Duration::from_secs(30)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(30)" "crates/vox-cli/src/commands/review/coderabbit/github/comments.rs"`

**Confidence**: high

---

### hv-0341 — `crates/vox-cli/src/commands/review/coderabbit/github/reviews/worktree.rs:46`

**Substring**

```text
Duration::from_millis(400)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(400)" "crates/vox-cli/src/commands/review/coderabbit/github/reviews/worktree.rs"`

**Confidence**: high

---

### hv-0342 — `crates/vox-cli/src/commands/share.rs:251`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-cli/src/commands/share.rs"`

**Confidence**: high

---

### hv-0343 — `crates/vox-cli/src/commands/share.rs:273`

**Substring**

```text
Duration::from_millis(200)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(200)" "crates/vox-cli/src/commands/share.rs"`

**Confidence**: high

---

### hv-0344 — `crates/vox-cli/src/commands/status.rs:32`

**Substring**

```text
Duration::from_millis(300)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(300)" "crates/vox-cli/src/commands/status.rs"`

**Confidence**: high

---

### hv-0345 — `crates/vox-cli/src/commands/test.rs:96`

**Substring**

```text
Duration::from_millis(300)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(300)" "crates/vox-cli/src/commands/test.rs"`

**Confidence**: high

---

### hv-0346 — `crates/vox-cli/src/compilerd.rs:329`

**Substring**

```text
Duration::from_secs(2)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(2)" "crates/vox-cli/src/compilerd.rs"`

**Confidence**: high

---

### hv-0347 — `crates/vox-cli/src/compilerd.rs:416`

**Substring**

```text
Duration::from_secs(2)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(2)" "crates/vox-cli/src/compilerd.rs"`

**Confidence**: high

---

### hv-0348 — `crates/vox-cli/src/frontend.rs:56`

**Substring**

```text
Duration::from_secs(2)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(2)" "crates/vox-cli/src/frontend.rs"`

**Confidence**: high

---

### hv-0349 — `crates/vox-cli/src/fs_utils.rs:175`

**Substring**

```text
Duration::from_secs(120)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(120)" "crates/vox-cli/src/fs_utils.rs"`

**Confidence**: high

---

### hv-0350 — `crates/vox-cli/src/progress.rs:40`

**Substring**

```text
Duration::from_millis(80)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(80)" "crates/vox-cli/src/progress.rs"`

**Confidence**: high

---

### hv-0351 — `crates/vox-cli/src/progress.rs:73`

**Substring**

```text
Duration::from_millis(80)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(80)" "crates/vox-cli/src/progress.rs"`

**Confidence**: high

---

### hv-0352 — `crates/vox-code-audit/src/review/client.rs:20`

**Substring**

```text
Duration::from_secs(120)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(120)" "crates/vox-code-audit/src/review/client.rs"`

**Confidence**: high

---

### hv-0353 — `crates/vox-codegen/src/codegen_rust/emit/http.rs:250`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-codegen/src/codegen_rust/emit/http.rs"`

**Confidence**: high

---

### hv-0354 — `crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs:950`

**Substring**

```text
setTimeout(() => resolve({ Error: "Timeout" }), 2000)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "setTimeout(() => resolve({ Error: \"Timeout\" }), 2000)" "crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs"`

**Confidence**: high

---

### hv-0355 — `crates/vox-db/src/circuit_breaker.rs:92`

**Substring**

```text
Duration::from_secs(30)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(30)" "crates/vox-db/src/circuit_breaker.rs"`

**Confidence**: high

---

### hv-0356 — `crates/vox-db/src/circuit_breaker.rs:175`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-db/src/circuit_breaker.rs"`

**Confidence**: high

---

### hv-0357 — `crates/vox-db/src/circuit_breaker.rs:186`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-db/src/circuit_breaker.rs"`

**Confidence**: high

---

### hv-0358 — `crates/vox-db/src/circuit_breaker.rs:203`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-db/src/circuit_breaker.rs"`

**Confidence**: high

---

### hv-0359 — `crates/vox-db/src/circuit_breaker.rs:215`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-db/src/circuit_breaker.rs"`

**Confidence**: high

---

### hv-0360 — `crates/vox-deploy-codegen/src/deploy_target.rs:527`

**Substring**

```text
Duration::from_secs(10)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(10)" "crates/vox-deploy-codegen/src/deploy_target.rs"`

**Confidence**: high

---

### hv-0361 — `crates/vox-drift-check/src/extractors/rust.rs:194`

**Substring**

```text
Duration::from_secs(30)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(30)" "crates/vox-drift-check/src/extractors/rust.rs"`

**Confidence**: high

---

### hv-0362 — `crates/vox-drift-check/src/extractors/rust.rs:205`

**Substring**

```text
Duration::from_millis(100)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(100)" "crates/vox-drift-check/src/extractors/rust.rs"`

**Confidence**: high

---

### hv-0363 — `crates/vox-gamify/src/output_policy.rs:71`

**Substring**

```text
Duration::from_secs(3600)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3600)" "crates/vox-gamify/src/output_policy.rs"`

**Confidence**: high

---

### hv-0364 — `crates/vox-http-client/src/lib.rs:20`

**Substring**

```text
Duration::from_secs(15)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(15)" "crates/vox-http-client/src/lib.rs"`

**Confidence**: high

---

### hv-0365 — `crates/vox-http-client/src/lib.rs:21`

**Substring**

```text
Duration::from_secs(90)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(90)" "crates/vox-http-client/src/lib.rs"`

**Confidence**: high

---

### hv-0366 — `crates/vox-ml-cli/src/commands/populi_cli.rs:941`

**Substring**

```text
Duration::from_secs(3600)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3600)" "crates/vox-ml-cli/src/commands/populi_cli.rs"`

**Confidence**: high

---

### hv-0367 — `crates/vox-ml-cli/src/commands/populi_cli.rs:1172`

**Substring**

```text
Duration::from_secs(3600)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3600)" "crates/vox-ml-cli/src/commands/populi_cli.rs"`

**Confidence**: high

---

### hv-0368 — `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:518`

**Substring**

```text
Duration::from_secs(2)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(2)" "crates/vox-ml-cli/src/commands/populi_lifecycle.rs"`

**Confidence**: high

---

### hv-0369 — `crates/vox-openclaw-runtime/src/openclaw_discovery.rs:156`

**Substring**

```text
Duration::from_secs(8)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(8)" "crates/vox-openclaw-runtime/src/openclaw_discovery.rs"`

**Confidence**: high

---

### hv-0370 — `crates/vox-openclaw-runtime/src/openclaw.rs:73`

**Substring**

```text
Duration::from_secs(60)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(60)" "crates/vox-openclaw-runtime/src/openclaw.rs"`

**Confidence**: high

---

### hv-0371 — `crates/vox-oratio/src/backends/cloud_offload.rs:25`

**Substring**

```text
Duration::from_secs(300)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(300)" "crates/vox-oratio/src/backends/cloud_offload.rs"`

**Confidence**: high

---

### hv-0372 — `crates/vox-orchestrator-mcp/src/http_gateway/eval.rs:110`

**Substring**

```text
Duration::from_secs(5)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(5)" "crates/vox-orchestrator-mcp/src/http_gateway/eval.rs"`

**Confidence**: high

---

### hv-0373 — `crates/vox-orchestrator-mcp/src/openclaw_tools.rs:152`

**Substring**

```text
Duration::from_secs(5)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(5)" "crates/vox-orchestrator-mcp/src/openclaw_tools.rs"`

**Confidence**: high

---

### hv-0374 — `crates/vox-orchestrator-mcp/src/server_state.rs:132`

**Substring**

```text
Duration::from_secs(120)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(120)" "crates/vox-orchestrator-mcp/src/server_state.rs"`

**Confidence**: high

---

### hv-0375 — `crates/vox-orchestrator-mcp/src/server_state.rs:182`

**Substring**

```text
Duration::from_secs(120)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(120)" "crates/vox-orchestrator-mcp/src/server_state.rs"`

**Confidence**: high

---

### hv-0376 — `crates/vox-orchestrator-mcp/src/visus_tools.rs:74`

**Substring**

```text
Duration::from_secs(3)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3)" "crates/vox-orchestrator-mcp/src/visus_tools.rs"`

**Confidence**: high

---

### hv-0377 — `crates/vox-orchestrator-mcp/src/visus_tools.rs:135`

**Substring**

```text
Duration::from_secs(3)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(3)" "crates/vox-orchestrator-mcp/src/visus_tools.rs"`

**Confidence**: high

---

### hv-0378 — `crates/vox-orchestrator/src/bulletin.rs:79`

**Substring**

```text
Duration::from_millis(100)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_millis(100)" "crates/vox-orchestrator/src/bulletin.rs"`

**Confidence**: high

---

### hv-0379 — `crates/vox-orchestrator/src/catalog.rs:19`

**Substring**

```text
Duration::from_secs(10)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(10)" "crates/vox-orchestrator/src/catalog.rs"`

**Confidence**: high

---

### hv-0380 — `crates/vox-orchestrator/src/catalog.rs:305`

**Substring**

```text
Duration::from_secs(5)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(5)" "crates/vox-orchestrator/src/catalog.rs"`

**Confidence**: high

---

### hv-0381 — `crates/vox-orchestrator/src/catalog.rs:387`

**Substring**

```text
Duration::from_secs(10)
```

**Why it matters**: Magic timeouts are hard to tune for slow networks or large models.

**Fix** (extract-named-constant): const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale

**Verify**: `rg -nF "Duration::from_secs(10)" "crates/vox-orchestrator/src/catalog.rs"`

**Confidence**: high

---

