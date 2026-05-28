// Vox Orchestrator GUI — fixtures rooted in real Vox surfaces:
// compiler/runtime, AgentOS (vox-orchestrator + vox-plugin-host), Mens/Populi (inference + QLoRA),
// Socrates protocol, Clavis (secrets), rule-pack, durable workflows, MCP tools, mesh.

window.voxTransport = {
  callTool: async (name, args = {}) => {
    console.log(`[voxTransport] ${name}`, args);
    await new Promise(r => setTimeout(r, 180 + Math.random() * 240));
    return { ok: true, tool: name, args, ts: Date.now() };
  },
  invoke: async (cmd, args = {}) => {
    console.log(`[tauri.invoke] ${cmd}`, args);
    await new Promise(r => setTimeout(r, 60));
    return { ok: true };
  }
};

window.VOX_FIXTURES = {
  // Top-bar KPIs: orchestrator + mesh + budget. Ludus demoted to a single small pip.
  kpis: {
    activeAgents: { value: 7,  delta: +2,   spark: [3,4,4,5,5,6,5,6,7,7] },
    queueDepth:   { value: 23, delta: -4,   spark: [31,29,30,28,27,26,25,24,24,23] },
    budgetBurn:   { value: 1.24, cap: 5.00, delta: +0.18, spark: [0.2,0.3,0.5,0.6,0.7,0.9,1.0,1.1,1.2,1.24] },
    mesh:         { value: 412, unit: "tok/s", peers: 4, vramGb: 96, delta: +38, spark: [180,210,240,280,310,340,360,380,400,412] },
    ludus:        { xp: 18420, streak: 12 }, // shown only in a small badge — not a HUD pillar
  },

  // Agents map to crates / files (file-affinity routing in vox-orchestrator).
  agents: [
    { id: "A-01", codename: "vox-compiler",     phase: "Verifying", task: "Lower @endpoint(kind: mutation) through pure-HIR",        crate: "crates/vox-compiler",         cost: 1.24, budget: 5.00, eta: "4m 12s", skill: "compiler",  progress: 0.71, doubts: 2 },
    { id: "A-02", codename: "vox-protocol",    phase: "Executing",  task: "Regenerate vox-client.ts from schema diff",                crate: "crates/vox-protocol",         cost: 0.42, budget: 2.50, eta: "2m 08s", skill: "codegen",    progress: 0.38, doubts: 0 },
    { id: "A-03", codename: "socrates",        phase: "Planning",   task: "Cross-check Pillar 3 deployment claims",                   crate: "vox-schola/scientia",         cost: 0.06, budget: 1.50, eta: "—",      skill: "research",   progress: 0.12, doubts: 1 },
    { id: "A-04", codename: "mens-candle-cuda",phase: "Executing",  task: "QLoRA epoch 7/12 · loss 1.84 → 1.71",                       crate: "crates/vox-inference",        cost: 0.88, budget: 3.00, eta: "6m 31s", skill: "training",   progress: 0.52, doubts: 0 },
    { id: "A-05", codename: "vox-arch-check",  phase: "Verifying",  task: "Layer audit · 101 crates · 0 inversions found",            crate: "crates/vox-arch-check",       cost: 0.31, budget: 1.00, eta: "1m 04s", skill: "audit",      progress: 0.84, doubts: 0 },
    { id: "A-06", codename: "vox-workflow-rt", phase: "Paused",     task: "Compaction · checkpoint log 4.2GB → 1.1GB",                 crate: "crates/vox-workflow-runtime", cost: 0.19, budget: 2.00, eta: "—",      skill: "durable",    progress: 0.27, doubts: 3 },
    { id: "A-07", codename: "vox-rule-pack",   phase: "Planning",   task: "Score new ai-laziness rule against fixtures",               crate: "crates/vox-rule-pack",        cost: 0.04, budget: 0.50, eta: "—",      skill: "audit",      progress: 0.08, doubts: 0 },
  ],

  // The Stream — real orchestrator events.
  stream: [
    { id: "ev-9821", ts: "14:02:11", kind: "validated",   agent: "A-05", title: "vox-arch-check passed",        body: "Layer graph clean across 101 crates. No fan-in violations. LoC budgets nominal.",         tag: "arch-check / Verifying" },
    { id: "ev-9820", ts: "14:01:47", kind: "in-progress", agent: "A-04", title: "Mens · QLoRA epoch 7/12",      body: "Loss 1.71 (Δ −0.13). Tok/s 412 · CUDA(2)+Metal(1) · adapter rank 16.",                    tag: "mens-candle-cuda / Executing" },
    { id: "ev-9819", ts: "14:01:12", kind: "doubted",    agent: "A-06", title: "Durable checkpoint stalled",   body: "vox-workflow-runtime · WAL compaction blocked on shard 0x3F. Awaiting overrule.",          tag: "workflow-rt / Paused" },
    { id: "ev-9818", ts: "14:00:33", kind: "validated",   agent: "A-02", title: "vox-client.ts regenerated",    body: "5 endpoints · 3 new mutations · diff +142 / −68. Type-check green on web target.",         tag: "vox-protocol / Executing" },
    { id: "ev-9817", ts: "13:59:58", kind: "speculative", agent: "A-03", title: "Socrates: source contested",   body: "Claim ‘compaction scaling in flight’ matched 2 of 3 retrieved sources. 67% citation cover.", tag: "socrates / Planning" },
    { id: "ev-9816", ts: "13:59:21", kind: "in-progress", agent: "A-01", title: "@endpoint lowering · pass 3",  body: "HIR rewrite for (kind: mutation). 18 nodes touched. LSP diagnostics propagated.",          tag: "compiler / Verifying" },
    { id: "ev-9815", ts: "13:58:40", kind: "validated",   agent: "A-05", title: "rule-pack fixtures · F1 0.94", body: "ai-laziness rule scored against 1,280 fixtures. Promotion gate cleared.",                  tag: "rule-pack / Verifying" },
    { id: "ev-9814", ts: "13:58:02", kind: "in-progress", agent: "A-04", title: "Mesh peer joined · workshop-2",body: "Apple M3 Max · 64GB unified · Metal backend registered. Capacity routed.",                  tag: "populi-mesh / Executing" },
  ],

  // System alerts (replaces gaming-forward Ludus pane). Telemetry, mesh, rule-pack, secrets.
  alerts: [
    { id: "al-401", level: "ok",     title: "Mesh capacity expanded",        body: "workshop-2 joined (Metal · 64GB). Mens routing accepting new lanes." },
    { id: "al-400", level: "warn",   title: "Budget gate · 24% of cap",       body: "vox-compiler approached soft cap. Auto-throttle engaged by policy module." },
    { id: "al-399", level: "info",   title: "Rule-pack v1.7 staged",          body: "ai-laziness detector promoted to v1. `vox audit` will pick it up on next run." },
    { id: "al-398", level: "info",   title: "Clavis · secret rotation due",   body: "ed25519 identity for ‘orchestrator’ at 87 days. Rotate before 14d window closes." },
  ],

  // First-party skills (Plugin Catalog, real names from the README).
  skills: [
    { id: "compiler",          name: "Compiler",          desc: "@endpoint, @table, @durable lowering through pure-HIR with LSP diagnostics.",       tier: "Mature",   deploys: 1428, glyph: "tdd" },
    { id: "codegen",           name: "Wire Codegen",       desc: "Emit React/TSX + vox-client.ts RPC bridge + deploy targets from one module graph.", tier: "Mature",   deploys: 962,  glyph: "refactor" },
    { id: "research",          name: "Socrates",           desc: "Autonomous retrieval + fact-checking with citation-bound claims.",                  tier: "Preview",  deploys: 511,  glyph: "research" },
    { id: "rag",               name: "Skill · RAG",        desc: "Persistent long-term memory with vector recall over agent history.",                tier: "Mature",   deploys: 712,  glyph: "memory" },
    { id: "audit",             name: "Rule Pack",          desc: "Stub, hollow-fn, victory-claim, ai-laziness, secret, magic-value detectors.",       tier: "Mature",   deploys: 631,  glyph: "triage" },
    { id: "durable",           name: "Durable Runtime",    desc: "Checkpointed execution under vox-workflow-runtime. Retry on fault, resume on node.", tier: "Preview", deploys: 384,  glyph: "writing" },
    { id: "training",          name: "Mens · Populi",      desc: "QLoRA fine-tunes on detected CUDA / Metal / WebGPU. Burn + Candle, no Python.",      tier: "Emergent", deploys: 173,  glyph: "crypto" },
    { id: "orchestrator",      name: "Orchestrator",       desc: "File-affinity routing + tier cascade, budget gate, circuit breaker, calibration.",  tier: "Stable",   deploys: 1840, glyph: "orchestrate" },
  ],

  // Policy modules of vox-orchestrator routing — replaces a generic "intention matrix".
  // Confidence = calibration score; phase = current state.
  policies: [
    { id: "pl-01", branch: "Tier Cascade",      conf: 0.94, phase: "Validated",   parent: "ROUTING", note: "Local-first; escalate to cloud only past confidence floor." },
    { id: "pl-02", branch: "File Affinity",     conf: 0.88, phase: "Active",      parent: "ROUTING", note: "Sticky agent assignment by crate path. Cache warm." },
    { id: "pl-03", branch: "Plan-Mode Trigger", conf: 0.71, phase: "Active",      parent: "ROUTING", note: "Force plan-mode when risk × cost crosses threshold." },
    { id: "pl-04", branch: "Risk Matrix",       conf: 0.81, phase: "Active",      parent: "SAFETY",  note: "Tier by shell-risk classifier (vox-exec-grammar) × capability set." },
    { id: "pl-05", branch: "Budget Gate",       conf: 0.66, phase: "Speculative", parent: "SAFETY",  note: "Soft cap warns; hard cap throttles. Per-agent and global." },
    { id: "pl-06", branch: "Circuit Breaker",   conf: 0.58, phase: "Doubted",     parent: "SAFETY",  note: "Trip on 3 consecutive failed verifies. Half-open at 60s." },
    { id: "pl-07", branch: "Calibration",       conf: 0.77, phase: "Active",      parent: "QUALITY", note: "Brier-score self-eval against rubric ground truth." },
    { id: "pl-08", branch: "Doubt Injection",   conf: 0.42, phase: "Speculative", parent: "QUALITY", note: "Inject Socrates into verify lane for high-risk diffs." },
    { id: "pl-09", branch: "Capability Gate",   conf: 0.88, phase: "Validated",   parent: "QUALITY", note: "vox-capability-registry + vox-identity ed25519 trust ledger." },
  ],

  // Mind-map: orchestrator routes work to agents (file-affinity).
  graph: {
    nodes: [
      { id: "A-01", label: "vox-compiler",   phase: "Verifying", x: 0.62, y: 0.30 },
      { id: "A-02", label: "vox-protocol",   phase: "Executing", x: 0.36, y: 0.58 },
      { id: "A-03", label: "socrates",       phase: "Planning",  x: 0.18, y: 0.30 },
      { id: "A-04", label: "mens-cuda",      phase: "Executing", x: 0.78, y: 0.62 },
      { id: "A-05", label: "arch-check",     phase: "Verifying", x: 0.50, y: 0.78 },
      { id: "A-06", label: "workflow-rt",    phase: "Paused",    x: 0.30, y: 0.12 },
      { id: "A-07", label: "rule-pack",      phase: "Planning",  x: 0.84, y: 0.20 },
      { id: "ROOT", label: "Orchestrator",   phase: "Root",      x: 0.50, y: 0.42 },
    ],
    edges: [
      { from: "ROOT", to: "A-01", flow: 0.9 },
      { from: "ROOT", to: "A-02", flow: 0.6 },
      { from: "ROOT", to: "A-03", flow: 0.3 },
      { from: "ROOT", to: "A-04", flow: 0.8 },
      { from: "A-01", to: "A-05", flow: 0.7 },
      { from: "A-02", to: "A-05", flow: 0.4 },
      { from: "A-03", to: "A-06", flow: 0.2 },
      { from: "A-04", to: "A-07", flow: 0.5 },
    ],
  },

  // Mesh peers (Distributed AgentOS).
  peers: [
    { id: "node-local",  name: "atelier (this)",  os: "macOS",  hw: "M3 Max · 64GB", backend: "Metal",  tok: 142, queue: 4, online: true },
    { id: "node-rig-1",  name: "rig-1",           os: "Linux",  hw: "RTX 4090 · 24GB", backend: "CUDA", tok: 188, queue: 7, online: true },
    { id: "node-rig-2",  name: "rig-2",           os: "Linux",  hw: "2× A6000 · 96GB", backend: "CUDA", tok: 82,  queue: 12, online: true },
    { id: "node-cloud",  name: "fly · iad",       os: "Linux",  hw: "CPU · 8 vCPU",    backend: "CPU",  tok: 0,   queue: 0, online: false },
  ],

  // Loquela context chips. Default to files & a skill — no agents, less gaming.
  contextChips: [
    { id: "f1", kind: "file",  label: "crates/vox-compiler/" },
    { id: "f2", kind: "file",  label: "examples/golden/tasks.vox" },
    { id: "s1", kind: "skill", label: "Compiler" },
  ],
};
