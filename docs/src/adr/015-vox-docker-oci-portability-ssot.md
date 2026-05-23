---
title: "ADR 015: Vox Docker/OCI portability SSOT"
description: "Formalizes Docker/OCI-backed application portability, layered SSOT boundaries, and the rule against a portability god object."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-28"
training_eligible: true

schema_type: "TechArticle"
---

## Status

Accepted.

## Context

Vox needs a practical cross-platform deployment model for `.vox` applications that:

- makes projects easy to package and distribute,
- reduces direct exposure to low-level host-OS variation,
- reuses mature deployment and artifact tooling,
- and fits the existing Vox package-management and deployment surfaces already present in-tree.

The repository already contains the main building blocks for this:

- `Vox.toml [deploy]` in `vox-pm`,
- `vox.lock` as the resolved-state package contract,
- `vox-container` with Docker/Podman runtime abstraction and deploy targets,
- deployment/operator docs under `docs/src/reference/`,
- and `vox-install-policy` as an example of a narrower SSOT for toolchain distribution.

The question is not whether Vox should support deployment. The question is where to place the portability boundary so Vox avoids taking on deep host-OS abstraction as a core language/runtime responsibility.

## Decision

Adopt a **Docker/OCI-backed portability model** as the primary deployment portability boundary for deployed `.vox` applications.

### Decision details

- `Vox.toml` is the **project desired-state** contract, including declarative deployment intent via `[deploy]`.
- `vox.lock` is the **project resolved-state** contract for reproducible packaging and deployment inputs.
- `vox-pm` owns dependency resolution, fetch, cache/CAS, materialization, and locked/offline/frozen policy semantics.
- `vox-container` owns runtime-specific packaging and deployment mechanics for OCI/container/compose/systemd/k8s targets.
- `contracts/cli/command-registry.yaml` remains the surfaced CLI contract and parity anchor.
- operator-facing portability rules live in the normative reference document `docs/src/reference/vox-portability-ssot.md`.
- `vox-install-policy` remains the SSOT for **toolchain portability** of the `vox` binary itself and is not merged into application portability policy.

### Explicit boundary rules

- Vox application portability is **not** implemented by a new central portability god object.
- Deep host-OS abstraction is out of scope for the primary application portability strategy.
- WASI/Wasmtime may remain a complementary script/isolation lane, but is **not** the primary portability boundary for deployed `.vox` applications.
- OCI registries are the preferred distribution substrate for deployable application artifacts and related metadata where appropriate.
- Docker is the primary documented portability abstraction; Podman compatibility remains important, especially for rootless/operator workflows.

## Consequences

### Positive

- Vox gains a realistic and widely supported portability boundary without claiming away kernel/runtime differences.
- Packaging, deployment, CI, and release policy can converge around one artifact model.
- Existing repo systems are extended instead of replaced.
- The architecture keeps clear ownership boundaries:
  - desired state,
  - resolved state,
  - materialization,
  - runtime/deploy execution,
  - operator/runtime contract.
- OCI ecosystem features such as multi-arch publication, annotations, SBOMs, provenance, signing, and registry storage become available without bespoke infrastructure.

> Nomenclature note (2026-05-08): `vox-pm` was renamed to `vox-package`; references in this ADR are historical.

### Trade-offs

- Portability claims must stay disciplined: containers do not erase kernel differences.
- Multi-arch publication and validation become part of the operational burden.
- CI and release flows gain additional policy complexity.
- Documentation must explicitly separate app portability from toolchain portability.
- Some current repo surfaces still need convergence before the architecture is fully reflected in code and command contracts.

## Consequences for implementation

- Future deployment work should extend `vox-pm`, `vox-container`, docs SSOTs, and CLI compliance surfaces rather than introducing a new orchestration layer.
- `vox.lock` must become deployment-relevant for reproducible packaging.
- The normative portability contract should be enforced gradually through CI and release gates.
- Deployment/operator docs should cite the portability SSOT for guarantees and caveats rather than rediscovering policy page by page.

## Related

- `docs/src/architecture/vox-docker-dotvox-portability-research-2026.md`
- `docs/src/architecture/vox-docker-dotvox-portability-implementation-plan-2026.md`
- `docs/src/reference/vox-portability-ssot.md`
- `docs/src/reference/deployment-compose.md`
- `crates/vox-pm/src/manifest.rs`
- `crates/vox-container/src/deploy_target.rs`
- `crates/vox-install-policy/src/lib.rs`

> **Nomenclature note (2026-05-08):** `vox-pm` was renamed to `vox-package`; references in this ADR are historical.

