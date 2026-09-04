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
  <img src="docs/src/assets/old_internet_knot_abstract.png" alt="A diagram illustrating the complexity of traditional web development fragmentation." width="80%" />
  <p>
    <strong>Fragmentation in Traditional Web Development</strong><br />
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
- [Wire format](crates/vox-foundation/) — Data and tool contracts are the single source of truth; schemas are generated, not restated.
- [Autonomous RAG & Research](docs/src/reference/socrates-protocol.md) — Deploy agents equipped with persistent long-term memory, fact-checking (the [Socrates protocol](docs/src/reference/socrates-protocol.md)), and autonomous web-search.

---

## Install

Vox is in pre-1.0 active development. `voxup` downloads a checksum-verified
release binary:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh
```

Windows, building from source, prerequisites, `vox doctor`, Docker, and the
optional subsystems: **[Installing Vox](docs/src/reference/installation.md)** —
the canonical page.

Homebrew, `.msi`, and `.deb` are **planned, not yet published**: the release
workflow has jobs for them, but the Homebrew tap update is a placeholder, the
MSI job has no binary to package, and the `.deb` is built but never uploaded.

### Quick Start
```bash
vox init my-app
cd my-app
vox run src/main.vox
```

## The CLI

The full CLI surface, including every `vox ci`, `vox populi`, and `vox mens` subcommand, lives at [`docs/src/reference/cli.md`](docs/src/reference/cli.md). Run `vox commands --recommended` for first-time discovery.

---

### Ecosystem & plugins

The core binary covers compile, run, bundle, and package. Heavier capabilities — Rust-native ML training/serving, the native desktop GUI, and 20+ bundled agent skills (git, memory, RAG, testing, container/WASM runtimes, and more) — load as optional extensions and skills through a stable ABI; `vox` tells you if one is required but missing.

Full extension and skill catalog, kept current automatically: **[Plugin Catalog](docs/src/reference/plugin-catalog.generated.md)**.

Project automation itself is `.vox`, not `.ps1`/`.sh`/`.py` — scripts are type-checked, cross-platform, and telemetry-observable by default (`vox run scripts/clean-build-artifacts.vox`).

Cross-machine orchestration (mesh) is opt-in: nodes advertise hardware capabilities on startup and the orchestrator routes workloads to the best-equipped peer, wire-checked at compile time. See the [model routing how-to](docs/src/how-to/how-to-model-routing.md).

---

## Stability & Path to 1.0

Vox is marching toward a production-hardened v1.0 release. Surfaces are graded by their architectural stability and proximity to the v1 criteria — a representative slice:

| Feature Area | Status |
|:---|:---|
| Compiler & LSP | 🟣 Mature |
| Database Engine | 🔵 Stable |
| Durable Runtime | 🔵 Stable (interpreter) / 🟡 Preview (codegen) |
| Native GUI (Tauri) | 🟡 Preview |
| Distributed Mesh | 🟠 Emergent |

Full per-surface matrix, all tiers explained, and v1.0 release criteria: **[voxlang.org/reference/stability](https://voxlang.org/reference/stability/)**.

Roadmap execution minimizes syntactic redundancy to stabilize the compiler primitives prior to v1.0. Retired symbols: [`AGENTS.md` retired-surfaces table](AGENTS.md).

---

## Documentation

Full docs, organized by intent (tutorials, how-to guides, reference, architecture): **https://voxlang.org**

---

## Contributing

Start at the [Contributor Hub](docs/src/contributors/contributor-hub.md). The [Contribution Loop](docs/src/contributors/contribution-loop.md) explains the write → verify → train cycle. If CI flags a gate failure, the [TOESTUB Guide](docs/src/contributors/toestub-contributor-guide.md) covers the common causes. Undocumented surfaces are tracked in [`DOC_GAPS.md`](docs/src/api/DOC_GAPS.md).

---

Beyond the rule pack, CI enforces repo-wide invariants — layer boundaries (`vox audit arch`), secret hygiene, generated-file drift, and more. Full detector inventory and rationale: [`AGENTS.md`](AGENTS.md).

---

## Backing, license, contact

Funded via [Open Collective](https://opencollective.com/vox-foundation) — every transaction is public. Sponsorships fund developer grants, MENS training hardware, and academic bounties.

[Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0): commercial use, patent grant, modification with attribution. [`LICENSE`](https://github.com/vox-foundation/vox/blob/main/LICENSE).

Discussion: [GitHub Discussions](https://github.com/vox-foundation/vox/discussions). Changelogs and ADRs: [RSS](https://voxlang.org/feed.xml).
