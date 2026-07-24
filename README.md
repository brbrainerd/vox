<div align="center">
  <img src="docs/src/assets/vox_hero_banner.jpeg" alt="Vox - The human voice acting as the great nerve of intelligence" width="100%" />

  <br><br>

  <p><strong>One <code>.vox</code> file compiles to a database schema, a typed server, a browser app, and the artifacts to deploy them.</strong> Initiated by Bertrand Reyna-Brainerd.</p>

  <p><a href="https://voxlang.org"><strong>voxlang.org</strong></a></p>
</div>

<p align="center">
  <a href="https://voxlang.org"><img src="https://img.shields.io/badge/docs-voxlang.org-blue?style=flat-square" alt="Documentation"/></a>
  <a href="https://github.com/vox-foundation/vox/commits/main"><img src="https://img.shields.io/github/last-commit/vox-foundation/vox?style=flat-square&label=updated" alt="Last Updated"/></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-green?style=flat-square" alt="License"/></a>
  <a href="https://voxlang.org/feed.xml"><img src="https://img.shields.io/badge/RSS-updates-orange?style=flat-square" alt="RSS Feed"/></a>
</p>

---

<!-- Code examples in this file mirror examples/golden/*.vox -->
<!-- Run: vox check examples/golden/*.vox to verify -->

<div align="center">
  <blockquote>
    <p><em>"Is it a fact — or have I dreamt it — that, by means of electricity, the world of matter has become a great nerve, vibrating thousands of miles in a breathless point of time? Rather, the round globe is a vast head, a brain, instinct with intelligence!"</em></p>
    <p>— Nathaniel Hawthorne, <em>The House of the Seven Gables</em> (1851)</p>
  </blockquote>
</div>

---

<!-- ANCHOR: why_vox -->
## Why Vox

Mainstream languages predate LLMs by decades. They tolerate implicit state — nulls, exceptions, schemas restated three times across the stack. That's tractable for a person; it's a minefield for a statistical code generator. A million-token context window doesn't help when most of it is integration boilerplate.

<div align="center">
  <img src="docs/src/assets/old_internet_knot_abstract.png" alt="A diagram illustrating the complexity of traditional web development fragmentation." width="80%">
  <p>
    <strong>Fragmentation in Traditional Web Development</strong><br>
    Traditional development requires restating data models and logic across frontend, API, backend, and database layers. This duplication creates significant maintenance overhead and increases the risk of integration drift.
  </p>
</div>

Vox is what falls out when you design the language *after* the model: collapse the duplications, push errors into the type system, draw the browser/server boundary in one place, and build durability and tool exposure into the grammar instead of layering them on top.
<!-- ANCHOR_END: why_vox -->

## Killer Features

Vox collapses the massive fragmentation of modern web and AI development into a single, cohesive ecosystem.

- **[Local AI Inference & Fine-Tuning](crates/vox-ml-cli/)**: Run models natively on your GPU without touching Python. Execute open-weights models or train them via QLoRA using Rust-native acceleration (CUDA and Apple Metal).
- **[One File to Rule the Stack](docs/src/reference/deployment-compose.md)**: A single `.vox` file emits database migrations, a typed API server, reactive frontend components, and deployment artifacts. Zero integration boilerplate.
- **[Distributed Mesh Computing](docs/src/how-to/how-to-model-routing.md)**: Securely network laptops and cloud servers. The orchestrator automatically routes AI workloads to the nodes with the best available hardware.
- **[Native Desktop GUI](crates/vox-gui/)**: Compile `.vox` files into fully native, cross-platform graphical applications powered by Tauri, complete with native IPC bridges.
- [Wire format](crates/vox-protocol/) — Data and tool contracts are the single source of truth; schemas are generated, not restated.
- [Autonomous RAG & Research](docs/src/reference/socrates-protocol.md) — Deploy agents equipped with persistent long-term memory, fact-checking (the [Socrates protocol](docs/src/reference/socrates-protocol.md)), and autonomous web-search.

---

## Install

Vox is currently in pre-1.0 active development. Official installation packages (`voxup`, `.msi`, `.deb`, Homebrew) are configured in the CI pipeline but have not yet been formally released. 

### Building from Source
**Prerequisites:** Ensure you have [Rust and Cargo](https://rustup.rs/) installed.

```bash
git clone https://github.com/vox-foundation/vox.git
cd vox
cargo install --path crates/vox-cli
```

### Quick Start
```bash
vox init my-app
cd my-app
vox run src/main.vox
```

## The CLI

The full CLI surface, including every `vox ci`, `vox populi`, and `vox mens` subcommand, lives at [`docs/src/reference/cli.md`](docs/src/reference/cli.md). Run `vox commands --recommended` for first-time discovery.

---

### Ecosystem & Plugins

Vox is highly modular. The core binary covers compile, run, bundle, and package; heavier capabilities are provided through optional extensions.

#### CLI Extensions
Extensions ship as separate binaries; `vox` will notify you if one is required but missing.

| Extension | Adds | Purpose |
|---|---|---|
| `vox-ml-cli` | `vox mens`, `vox populi`, ... | Rust-native ML (Candle, Whisper) for training and serving. |
| `vox-gui` | `vox gui` | Native Tauri desktop application environment. |

#### Agent Skills
The Vox AgentOS dynamically loads capabilities (Skills) through a stable ABI. First-party skills include:

- **ML & Audio**: `mens-candle-cuda`, `mens-candle-metal`, `oratio-mic`, `populi-mesh`
- **Execution**: `runtime-container` (Docker), `runtime-wasm`, `script-execution`
- **Agent Skills**: `skill-git`, `skill-memory`, `skill-orchestrator`, `skill-rag`, `skill-testing`
- **Infrastructure**: `api`, `cloud`, `host`, `types`, `webhook`

*→ See the [Plugin Catalog](docs/src/reference/plugin-catalog.generated.md) for detailed signatures.*

## How Vox works

One `.vox` file becomes a database schema, a type-safe server, and a live browser UI. Full walkthrough of all five architectural pillars: **https://voxlang.org/#how-vox-works**

---

## Automation: VoxScript-first

Project automation is `.vox`, not `.ps1`, `.sh`, or `.py`. Vox scripts are type-checked, cross-platform, and telemetry-observable by default.

```bash
vox run scripts/clean-cache.vox
vox run --isolation wasm scripts/process-untrusted-data.vox
```

**Key Commands:**
- `vox share publish` — Create a short-lived public tunnel for local previews.
- `vox audit` — Run the rule-pack against your codebase.
- `vox telemetry doctor` — Verify sink health and event wiring.

---

## Distributed AgentOS (Mesh)

Cross-machine orchestration is opt-in. Nodes advertise hardware capabilities (CUDA/Metal/VRAM) on startup, and the orchestrator automatically routes workloads to the best-equipped peer. Agent-to-agent communication is type-safe; wire mismatches are caught at compile time.

```bash
VOX_MESH_ENABLED=1 VOX_MESH_NODE_ID=my-node vox populi serve
vox populi status --quotas
```

Local models and cloud providers are managed through a unified policy layer with per-node quotas. See the [model routing how-to](docs/src/how-to/how-to-model-routing.md).

---

## Stability & Path to 1.0

<!-- ANCHOR: tier_table -->
Vox is marching toward a production-hardened v1.0 release. Surfaces are graded by their architectural stability and proximity to the v1 criteria.

| Feature Area | Status | Context & Maturity |
|:---|:---|:---|
| **Core Intelligence** | | |
| Orchestrator Core | 🔵 Stable | Thread-safe dispatch, agent lifecycle, and [Superpowers](docs/src/architecture/superpowers-ssot.md) orchestration. |
| Agent Skills (MCP) | 🟣 Mature | Full [MCP v1.0](https://modelcontextprotocol.io) compliance with 100+ first-party tools. |
| Socrates Research | 🟡 Preview | [Socrates protocol](docs/src/reference/socrates-protocol.md) for automated fact-checking and retrieval. |
| **Language Platform** | | |
| Compiler Core | 🟣 Mature | Wave 2 complete: pure-HIR lowering and stable syntax grammar. |
| LSP & IDE Tools | 🟣 Mature | Production-grade `vox-lsp` with full cross-reference support. |
| Durable Runtime | 🔵 Stable (interpreter) / 🟡 Preview (codegen) | Interpreter path: journal-backed replay for the supported subset ([ADR-019](docs/src/adr/019-durable-workflow-journal-contract-v1.md) / [ADR-021](docs/src/adr/021-generated-workflow-durability-parity.md)) ships; `@scheduled` runs on a persistent scheduler with crash-safe state. Codegen path: generated binaries link but `current_hir_module()` registration in emitted `main()` is a tracked Phase-5 follow-up — see [ADR-041](docs/src/adr/041-durable-functions-completion-2026.md). Unrestricted control-flow replay is explicit non-goal. |
| **Data & Foundation** | | |
| Database Engine | 🔵 Stable | [vox-db](crates/vox-db/) with Turso integration and zero-downtime migrations. |
| Secrets & Safety | 🔵 Stable | [Clavis](crates/vox-secrets/) hardened vault and [Rule Pack](crates/vox-rule-pack/) CI guards. |
| Telemetry Facade | 🟣 Mature | Unified [vox-telemetry](crates/vox-telemetry/) with trace propagation and cost rollups. |
| **AI/ML Engine** | | |
| Inference (Mens) | 🟡 Preview | Native CUDA/Metal/CPU inference with [Candle/Burn](crates/vox-populi/src/inference/mod.rs). |
| Training (Populi) | 🟠 Emergent | QLoRA native pipeline; loss-parity verification in progress. |
| Visus (Vision) | 🟠 Emergent | [Voice of Vision](crates/vox-cli/src/commands/visus/) for automated GUI bug detection. |
| **Platform & UI** | | |
| CLI & DX | 🟣 Mature | Rich diagnostic surface (`vox audit`, `vox ci`, `vox drift-check`). |
| Native GUI (Tauri) | 🟡 Preview | Tauri 2.0 integration with Dashboard, Agent Flow, and Superpowers catalog. |
| Distributed Mesh | 🟠 Emergent | Node discovery and workload routing functional across peers. |

**Stability Tiers:**
- 🟢 **Production Candidate**: Hardened for 1.0; feature-complete and regression-free.
- 🔵 **Stable**: API locked; high test coverage; used in core production internal loops.
- 🟣 **Mature**: Core logic stable; focus on ergonomics, documentation, and performance.
- 🟡 **Preview**: Feature-complete; API may shift based on early adopter feedback.
- 🟠 **Emergent**: Core logic functional; major feature parity or scaling remaining.
- 🚧 **Experimental**: Proof of concept; breaking changes are frequent and expected.
- 🔬 **Research**: Internal prototypes; not yet exposed in the standard CLI surface.

v1.0 criteria: [`docs/src/architecture/v1-release-criteria.md`](docs/src/architecture/v1-release-criteria.md). Roadmap: [GUI-native phases](docs/src/architecture/gui-native-roadmap-status-2026.md). History: [`CHANGELOG.md`](CHANGELOG.md).
<!-- ANCHOR_END: tier_table -->

Roadmap execution minimizes syntactic redundancy to stabilize the compiler primitives prior to v1.0. Retired symbols: [`AGENTS.md` retired-surfaces table](AGENTS.md).

---

## Documentation

Full docs, organized by intent (tutorials, how-to guides, reference, architecture): **https://voxlang.org**

---

## Contributing

Start at the [Contributor Hub](docs/src/contributors/contributor-hub.md). The [Contribution Loop](docs/src/contributors/contribution-loop.md) explains the write → verify → train cycle. If CI flags a gate failure, the [TOESTUB Guide](docs/src/contributors/toestub-contributor-guide.md) covers the common causes. Undocumented surfaces are tracked in [`DOC_GAPS.md`](docs/src/api/DOC_GAPS.md).

---

Beyond the rule pack, the CI environment enforces repo-wide invariants:

| Guard | Purpose |
|---|---|
| `vox arch-check` | Blocks layer inversions, fan-in violations, and LoC budget overruns. |
| `vox ci secret-env-guard` | Blocks raw `std::env` calls; enforces `vox-secrets` as the only secret source. |
| `vox ci sync-ignore-files` | Ensures `.voxignore` is synced to IDE-specific ignore files. |
| `vox-drift-check` | Detects semantic drift in multi-language workspace artifacts. |

Rationale and the full detector inventory live in [`AGENTS.md`](AGENTS.md).

---

## Backing, license, contact

Funded via [Open Collective](https://opencollective.com/vox-foundation) — every transaction is public. Sponsorships fund developer grants, MENS training hardware, and academic bounties.

[Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0): commercial use, patent grant, modification with attribution. [`LICENSE`](https://github.com/vox-foundation/vox/blob/main/LICENSE).

Discussion: [GitHub Discussions](https://github.com/vox-foundation/vox/discussions). Changelogs and ADRs: [RSS](https://voxlang.org/feed.xml).

---

## References

<a id="ref1"></a>**[1]** Fateev, M., & Abbas, S. (2019). *Temporal*. Temporal Technologies. <https://temporal.io>

<a id="ref2"></a>**[2]** Armstrong, J. (2003). *Making reliable distributed systems in the presence of software errors* [Ph.D. thesis, Royal Institute of Technology, Stockholm]. <https://erlang.org/download/armstrong_thesis_2003.pdf>

<a id="ref3"></a>**[3]** Anthropic. (2024). *Model Context Protocol*. <https://modelcontextprotocol.io>

<a id="ref4"></a>**[4]** Dettmers, T., Pagnoni, A., Holtzman, A., & Zettlemoyer, L. (2023). *QLoRA: Efficient Finetuning of Quantized LLMs*. arXiv. <https://arxiv.org/abs/2305.14314>

<a id="ref5"></a>**[5]** Earley, J. (1970). *An efficient context-free parsing algorithm*. Communications of the ACM, 13(2), 94-102. <https://dl.acm.org/doi/10.1145/362007.362035>
