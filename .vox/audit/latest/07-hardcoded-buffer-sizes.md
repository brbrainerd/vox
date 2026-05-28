# 07 — hardcoded buffer sizes

**Severity**: warning  
**Itemized**: 100

### hv-0425 — `contracts/reports/scaling-audit/findings-latest.json:26819`

**Substring**

```text
BufReader::with_capacity(128
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "BufReader::with_capacity(128" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0426 — `contracts/reports/scaling-audit/findings-latest.json:27529`

**Substring**

```text
channel(1024)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "channel(1024)" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0427 — `contracts/reports/scaling-audit/findings-latest.json:28057`

**Substring**

```text
with_capacity(4096)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(4096)" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0428 — `contracts/reports/scaling-audit/findings-latest.json:28069`

**Substring**

```text
with_capacity(8192)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(8192)" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0429 — `contracts/reports/scaling-audit/findings-latest.json:28092`

**Substring**

```text
with_capacity(8192)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(8192)" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0430 — `contracts/reports/scaling-audit/findings-latest.json:28104`

**Substring**

```text
with_capacity(4096)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(4096)" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0431 — `crates/vox-actor-runtime/src/routing_telemetry.rs:103`

**Substring**

```text
bounded(200)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "bounded(200)" "crates/vox-actor-runtime/src/routing_telemetry.rs"`

**Confidence**: medium

---

### hv-0432 — `crates/vox-arch-check/src/main.rs:1058`

**Substring**

```text
[0u8; 4096]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 4096]" "crates/vox-arch-check/src/main.rs"`

**Confidence**: medium

---

### hv-0433 — `crates/vox-cli/src/commands/mcp_server/wasm.rs:168`

**Substring**

```text
channel(100)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "channel(100)" "crates/vox-cli/src/commands/mcp_server/wasm.rs"`

**Confidence**: medium

---

### hv-0434 — `crates/vox-cli/src/commands/mcp_server/wasm.rs:169`

**Substring**

```text
channel(100)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "channel(100)" "crates/vox-cli/src/commands/mcp_server/wasm.rs"`

**Confidence**: medium

---

### hv-0435 — `crates/vox-cli/src/commands/secrets.rs:765`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-cli/src/commands/secrets.rs"`

**Confidence**: medium

---

### hv-0436 — `crates/vox-cli/src/commands/workflow/drain.rs:30`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-cli/src/commands/workflow/drain.rs"`

**Confidence**: medium

---

### hv-0437 — `crates/vox-code-audit/src/detectors/victory_claim.rs:21`

**Substring**

```text
with_capacity(4)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(4)" "crates/vox-code-audit/src/detectors/victory_claim.rs"`

**Confidence**: medium

---

### hv-0438 — `crates/vox-corpus/src/training/preflight.rs:41`

**Substring**

```text
BufReader::with_capacity(128
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "BufReader::with_capacity(128" "crates/vox-corpus/src/training/preflight.rs"`

**Confidence**: medium

---

### hv-0439 — `crates/vox-crypto/src/facades.rs:34`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-crypto/src/facades.rs"`

**Confidence**: medium

---

### hv-0440 — `crates/vox-crypto/src/facades.rs:43`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-crypto/src/facades.rs"`

**Confidence**: medium

---

### hv-0441 — `crates/vox-crypto/src/facades.rs:198`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-crypto/src/facades.rs"`

**Confidence**: medium

---

### hv-0442 — `crates/vox-crypto/src/facades.rs:202`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-crypto/src/facades.rs"`

**Confidence**: medium

---

### hv-0443 — `crates/vox-crypto/src/facades.rs:295`

**Substring**

```text
[0u8; 11]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 11]" "crates/vox-crypto/src/facades.rs"`

**Confidence**: medium

---

### hv-0444 — `crates/vox-crypto/src/facades.rs:303`

**Substring**

```text
[0u8; 13]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 13]" "crates/vox-crypto/src/facades.rs"`

**Confidence**: medium

---

### hv-0445 — `crates/vox-crypto/src/facades.rs:317`

**Substring**

```text
[0u8; 11]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 11]" "crates/vox-crypto/src/facades.rs"`

**Confidence**: medium

---

### hv-0446 — `crates/vox-crypto/src/facades.rs:328`

**Substring**

```text
[0u8; 12]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 12]" "crates/vox-crypto/src/facades.rs"`

**Confidence**: medium

---

### hv-0447 — `crates/vox-db/src/writer_actor.rs:255`

**Substring**

```text
channel(1024)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "channel(1024)" "crates/vox-db/src/writer_actor.rs"`

**Confidence**: medium

---

### hv-0448 — `crates/vox-distributed-training/src/checkpoint.rs:93`

**Substring**

```text
[1u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[1u8; 64]" "crates/vox-distributed-training/src/checkpoint.rs"`

**Confidence**: medium

---

### hv-0449 — `crates/vox-distributed-training/src/checkpoint.rs:94`

**Substring**

```text
[2u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[2u8; 64]" "crates/vox-distributed-training/src/checkpoint.rs"`

**Confidence**: medium

---

### hv-0450 — `crates/vox-distributed-training/src/checkpoint.rs:103`

**Substring**

```text
[4u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[4u8; 64]" "crates/vox-distributed-training/src/checkpoint.rs"`

**Confidence**: medium

---

### hv-0451 — `crates/vox-distributed-training/src/gradient.rs:67`

**Substring**

```text
[7u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[7u8; 64]" "crates/vox-distributed-training/src/gradient.rs"`

**Confidence**: medium

---

### hv-0452 — `crates/vox-distributed-training/src/strategy/data_parallel.rs:37`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-distributed-training/src/strategy/data_parallel.rs"`

**Confidence**: medium

---

### hv-0453 — `crates/vox-grammar-export/src/compact_prompt.rs:20`

**Substring**

```text
with_capacity(4096)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(4096)" "crates/vox-grammar-export/src/compact_prompt.rs"`

**Confidence**: medium

---

### hv-0454 — `crates/vox-grammar-export/src/ebnf.rs:14`

**Substring**

```text
with_capacity(8192)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(8192)" "crates/vox-grammar-export/src/ebnf.rs"`

**Confidence**: medium

---

### hv-0455 — `crates/vox-grammar-export/src/lark.rs:11`

**Substring**

```text
with_capacity(8192)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(8192)" "crates/vox-grammar-export/src/lark.rs"`

**Confidence**: medium

---

### hv-0456 — `crates/vox-grammar-export/src/ssot_markdown.rs:71`

**Substring**

```text
with_capacity(4096)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(4096)" "crates/vox-grammar-export/src/ssot_markdown.rs"`

**Confidence**: medium

---

### hv-0457 — `crates/vox-identity/src/challenge.rs:5`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-identity/src/challenge.rs"`

**Confidence**: medium

---

### hv-0458 — `crates/vox-identity/src/storage.rs:17`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-identity/src/storage.rs"`

**Confidence**: medium

---

### hv-0459 — `crates/vox-identity/src/storage.rs:26`

**Substring**

```text
[0u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 16]" "crates/vox-identity/src/storage.rs"`

**Confidence**: medium

---

### hv-0460 — `crates/vox-identity/src/storage.rs:31`

**Substring**

```text
[0u8; 12]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 12]" "crates/vox-identity/src/storage.rs"`

**Confidence**: medium

---

### hv-0461 — `crates/vox-identity/src/storage.rs:92`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-identity/src/storage.rs"`

**Confidence**: medium

---

### hv-0462 — `crates/vox-inference/src/dispatcher.rs:57`

**Substring**

```text
[1u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[1u8; 64]" "crates/vox-inference/src/dispatcher.rs"`

**Confidence**: medium

---

### hv-0463 — `crates/vox-inference/src/dispatcher.rs:59`

**Substring**

```text
[2u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[2u8; 64]" "crates/vox-inference/src/dispatcher.rs"`

**Confidence**: medium

---

### hv-0464 — `crates/vox-inference/src/dispatcher.rs:60`

**Substring**

```text
[3u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[3u8; 64]" "crates/vox-inference/src/dispatcher.rs"`

**Confidence**: medium

---

### hv-0465 — `crates/vox-inference/src/dispatcher.rs:61`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-inference/src/dispatcher.rs"`

**Confidence**: medium

---

### hv-0466 — `crates/vox-mesh-types/src/trace.rs:29`

**Substring**

```text
[0u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 16]" "crates/vox-mesh-types/src/trace.rs"`

**Confidence**: medium

---

### hv-0467 — `crates/vox-mesh-types/src/trace.rs:40`

**Substring**

```text
[0u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 16]" "crates/vox-mesh-types/src/trace.rs"`

**Confidence**: medium

---

### hv-0468 — `crates/vox-mesh-types/src/trace.rs:49`

**Substring**

```text
[0u8; 8]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 8]" "crates/vox-mesh-types/src/trace.rs"`

**Confidence**: medium

---

### hv-0469 — `crates/vox-mesh-types/src/trace.rs:60`

**Substring**

```text
[0u8; 8]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 8]" "crates/vox-mesh-types/src/trace.rs"`

**Confidence**: medium

---

### hv-0470 — `crates/vox-ml-cli/src/commands/ai/serve/handlers.rs:195`

**Substring**

```text
channel(32)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "channel(32)" "crates/vox-ml-cli/src/commands/ai/serve/handlers.rs"`

**Confidence**: medium

---

### hv-0471 — `crates/vox-nanopub/src/signing.rs:23`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-nanopub/src/signing.rs"`

**Confidence**: medium

---

### hv-0472 — `crates/vox-orchestrator-mcp/src/http_gateway/mod.rs:704`

**Substring**

```text
[0u8; 256]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 256]" "crates/vox-orchestrator-mcp/src/http_gateway/mod.rs"`

**Confidence**: medium

---

### hv-0473 — `crates/vox-orchestrator-mcp/src/http_gateway/mod.rs:710`

**Substring**

```text
[0u8; 512]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 512]" "crates/vox-orchestrator-mcp/src/http_gateway/mod.rs"`

**Confidence**: medium

---

### hv-0474 — `crates/vox-orchestrator-mcp/src/http_gateway/token.rs:37`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-orchestrator-mcp/src/http_gateway/token.rs"`

**Confidence**: medium

---

### hv-0475 — `crates/vox-orchestrator-mcp/src/server_state.rs:154`

**Substring**

```text
channel(256)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "channel(256)" "crates/vox-orchestrator-mcp/src/server_state.rs"`

**Confidence**: medium

---

### hv-0476 — `crates/vox-orchestrator-mcp/src/server_state.rs:204`

**Substring**

```text
channel(256)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "channel(256)" "crates/vox-orchestrator-mcp/src/server_state.rs"`

**Confidence**: medium

---

### hv-0477 — `crates/vox-orchestrator-mcp/src/server_state.rs:474`

**Substring**

```text
channel(256)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "channel(256)" "crates/vox-orchestrator-mcp/src/server_state.rs"`

**Confidence**: medium

---

### hv-0478 — `crates/vox-orchestrator-queue/src/affinity.rs:386`

**Substring**

```text
[1u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[1u8; 16]" "crates/vox-orchestrator-queue/src/affinity.rs"`

**Confidence**: medium

---

### hv-0479 — `crates/vox-orchestrator-queue/src/affinity.rs:387`

**Substring**

```text
[2u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[2u8; 16]" "crates/vox-orchestrator-queue/src/affinity.rs"`

**Confidence**: medium

---

### hv-0480 — `crates/vox-orchestrator-queue/src/affinity.rs:418`

**Substring**

```text
[5u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[5u8; 16]" "crates/vox-orchestrator-queue/src/affinity.rs"`

**Confidence**: medium

---

### hv-0481 — `crates/vox-orchestrator-queue/src/affinity.rs:432`

**Substring**

```text
[1u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[1u8; 16]" "crates/vox-orchestrator-queue/src/affinity.rs"`

**Confidence**: medium

---

### hv-0482 — `crates/vox-orchestrator-queue/src/affinity.rs:433`

**Substring**

```text
[2u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[2u8; 16]" "crates/vox-orchestrator-queue/src/affinity.rs"`

**Confidence**: medium

---

### hv-0483 — `crates/vox-orchestrator-queue/src/affinity.rs:444`

**Substring**

```text
[1u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[1u8; 16]" "crates/vox-orchestrator-queue/src/affinity.rs"`

**Confidence**: medium

---

### hv-0484 — `crates/vox-orchestrator-queue/src/affinity.rs:445`

**Substring**

```text
[2u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[2u8; 16]" "crates/vox-orchestrator-queue/src/affinity.rs"`

**Confidence**: medium

---

### hv-0485 — `crates/vox-orchestrator-queue/src/oplog/checkpoint.rs:16`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-orchestrator-queue/src/oplog/checkpoint.rs"`

**Confidence**: medium

---

### hv-0486 — `crates/vox-orchestrator-queue/src/oplog/persist.rs:57`

**Substring**

```text
[0u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 16]" "crates/vox-orchestrator-queue/src/oplog/persist.rs"`

**Confidence**: medium

---

### hv-0487 — `crates/vox-orchestrator-queue/src/oplog/persist.rs:160`

**Substring**

```text
[0u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 16]" "crates/vox-orchestrator-queue/src/oplog/persist.rs"`

**Confidence**: medium

---

### hv-0488 — `crates/vox-orchestrator-queue/src/oplog/query.rs:66`

**Substring**

```text
[0u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 16]" "crates/vox-orchestrator-queue/src/oplog/query.rs"`

**Confidence**: medium

---

### hv-0489 — `crates/vox-orchestrator-queue/src/oplog/sign.rs:88`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-orchestrator-queue/src/oplog/sign.rs"`

**Confidence**: medium

---

### hv-0490 — `crates/vox-orchestrator-queue/src/oplog/sign.rs:125`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-orchestrator-queue/src/oplog/sign.rs"`

**Confidence**: medium

---

### hv-0491 — `crates/vox-orchestrator-queue/src/oplog/store.rs:182`

**Substring**

```text
[0u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 16]" "crates/vox-orchestrator-queue/src/oplog/store.rs"`

**Confidence**: medium

---

### hv-0492 — `crates/vox-orchestrator-types/src/merge_outcome.rs:82`

**Substring**

```text
[7u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[7u8; 16]" "crates/vox-orchestrator-types/src/merge_outcome.rs"`

**Confidence**: medium

---

### hv-0493 — `crates/vox-orchestrator-types/src/merge_outcome.rs:108`

**Substring**

```text
[0u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 16]" "crates/vox-orchestrator-types/src/merge_outcome.rs"`

**Confidence**: medium

---

### hv-0494 — `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs:333`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs"`

**Confidence**: medium

---

### hv-0495 — `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs:359`

**Substring**

```text
[1u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[1u8; 16]" "crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs"`

**Confidence**: medium

---

### hv-0496 — `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs:360`

**Substring**

```text
[2u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[2u8; 16]" "crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs"`

**Confidence**: medium

---

### hv-0497 — `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs:374`

**Substring**

```text
[3u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[3u8; 16]" "crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs"`

**Confidence**: medium

---

### hv-0498 — `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs:392`

**Substring**

```text
[7u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[7u8; 16]" "crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs"`

**Confidence**: medium

---

### hv-0499 — `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs:410`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs"`

**Confidence**: medium

---

### hv-0500 — `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs:411`

**Substring**

```text
[9u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[9u8; 32]" "crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs"`

**Confidence**: medium

---

### hv-0501 — `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs:412`

**Substring**

```text
[4u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[4u8; 16]" "crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs"`

**Confidence**: medium

---

### hv-0502 — `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs:419`

**Substring**

```text
[9u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[9u8; 32]" "crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs"`

**Confidence**: medium

---

### hv-0503 — `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs:425`

**Substring**

```text
[0u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 16]" "crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs"`

**Confidence**: medium

---

### hv-0504 — `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs:429`

**Substring**

```text
[2u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[2u8; 16]" "crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs"`

**Confidence**: medium

---

### hv-0505 — `crates/vox-orchestrator/src/a2a/jwe.rs:32`

**Substring**

```text
[0u8; 12]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 12]" "crates/vox-orchestrator/src/a2a/jwe.rs"`

**Confidence**: medium

---

### hv-0506 — `crates/vox-orchestrator/src/a2a/traceparent.rs:23`

**Substring**

```text
[0u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 16]" "crates/vox-orchestrator/src/a2a/traceparent.rs"`

**Confidence**: medium

---

### hv-0507 — `crates/vox-orchestrator/src/a2a/traceparent.rs:24`

**Substring**

```text
[0u8; 8]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 8]" "crates/vox-orchestrator/src/a2a/traceparent.rs"`

**Confidence**: medium

---

### hv-0508 — `crates/vox-orchestrator/src/dei_shim/research/emitter.rs:33`

**Substring**

```text
channel(4)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "channel(4)" "crates/vox-orchestrator/src/dei_shim/research/emitter.rs"`

**Confidence**: medium

---

### hv-0509 — `crates/vox-orchestrator/src/mesh.rs:347`

**Substring**

```text
[0u8; 16]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 16]" "crates/vox-orchestrator/src/mesh.rs"`

**Confidence**: medium

---

### hv-0510 — `crates/vox-orchestrator/src/mesh.rs:365`

**Substring**

```text
with_capacity(22)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(22)" "crates/vox-orchestrator/src/mesh.rs"`

**Confidence**: medium

---

### hv-0511 — `crates/vox-orchestrator/src/tool_receipt.rs:47`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-orchestrator/src/tool_receipt.rs"`

**Confidence**: medium

---

### hv-0512 — `crates/vox-orchestrator/src/tool_receipt.rs:61`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-orchestrator/src/tool_receipt.rs"`

**Confidence**: medium

---

### hv-0513 — `crates/vox-package/src/bundle.rs:34`

**Substring**

```text
with_capacity(128)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(128)" "crates/vox-package/src/bundle.rs"`

**Confidence**: medium

---

### hv-0514 — `crates/vox-package/src/bundle.rs:64`

**Substring**

```text
with_capacity(128)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(128)" "crates/vox-package/src/bundle.rs"`

**Confidence**: medium

---

### hv-0515 — `crates/vox-package/src/bundle.rs:76`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-package/src/bundle.rs"`

**Confidence**: medium

---

### hv-0516 — `crates/vox-package/src/model_bundle.rs:19`

**Substring**

```text
with_capacity(128)
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "with_capacity(128)" "crates/vox-package/src/model_bundle.rs"`

**Confidence**: medium

---

### hv-0517 — `crates/vox-package/src/model_bundle.rs:35`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-package/src/model_bundle.rs"`

**Confidence**: medium

---

### hv-0518 — `crates/vox-package/src/model_bundle.rs:162`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-package/src/model_bundle.rs"`

**Confidence**: medium

---

### hv-0519 — `crates/vox-plugin-mens-candle-cuda/src/qlora_preflight.rs:69`

**Substring**

```text
[0u8; 8]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 8]" "crates/vox-plugin-mens-candle-cuda/src/qlora_preflight.rs"`

**Confidence**: medium

---

### hv-0520 — `crates/vox-plugin-mens-candle-cuda/src/qlora_weights.rs:94`

**Substring**

```text
[0u8; 8]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 8]" "crates/vox-plugin-mens-candle-cuda/src/qlora_weights.rs"`

**Confidence**: medium

---

### hv-0521 — `crates/vox-plugin-mens-candle-metal/src/qlora_preflight.rs:69`

**Substring**

```text
[0u8; 8]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 8]" "crates/vox-plugin-mens-candle-metal/src/qlora_preflight.rs"`

**Confidence**: medium

---

### hv-0522 — `crates/vox-plugin-mens-candle-metal/src/qlora_weights.rs:94`

**Substring**

```text
[0u8; 8]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 8]" "crates/vox-plugin-mens-candle-metal/src/qlora_weights.rs"`

**Confidence**: medium

---

### hv-0523 — `crates/vox-plugin-populi-mesh/src/transport/handlers/federation.rs:45`

**Substring**

```text
[0u8; 64]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 64]" "crates/vox-plugin-populi-mesh/src/transport/handlers/federation.rs"`

**Confidence**: medium

---

### hv-0524 — `crates/vox-plugin-populi-mesh/src/transport/mod.rs:545`

**Substring**

```text
[0u8; 32]
```

**Why it matters**: Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.

**Fix** (extract-named-constant): const BUF_CAP: usize = …; // name ties size to protocol / device limits

**Verify**: `rg -nF "[0u8; 32]" "crates/vox-plugin-populi-mesh/src/transport/mod.rs"`

**Confidence**: medium

---

