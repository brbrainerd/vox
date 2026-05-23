---
title: "Vox Docker-backed portability research 2026"
description: "Research findings and architectural recommendation for Docker/OCI-backed cross-platform `.vox` deployment, packaging, and SSOT boundaries."
category: "Architecture SSOTs"
last_updated: "2026-03-28"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

## Decision context

One Vox design goal is that a `.vox` program should be easy to package, easy to distribute, and easy to execute on heterogeneous systems without forcing the language/runtime surface to absorb every low-level operating-system difference directly.

The intended product experience is:

- authors declare project and deploy intent once,
- `vox` handles the packaging and runtime mechanics mostly behind the scenes,
- operators can run the result on common hosts without bespoke per-OS assembly,
- and the same project contract scales from local development to CI to deployment.

This document evaluates how to realize that goal by extending existing Vox systems rather than introducing a new portability framework.

## Executive recommendation

Vox should standardize on a **Docker/OCI-backed portability model** for deployed `.vox` applications, with **`Vox.toml` + `vox.lock`** as the project-level source of truth and **`vox-container`** as the execution/deployment engine.

That means:

- **`Vox.toml`** declares desired state, including deployment intent via `[deploy]`.
- **`vox.lock`** binds the resolved dependency graph and build inputs needed for reproducible packaging.
- **`vox-pm`** owns resolution, fetch, cache/CAS, and materialization.
- **`vox-container`** owns runtime-specific packaging/execution mechanics for OCI/container/compose/systemd/k8s targets.
- **OCI registries** become the preferred distribution substrate for deployable outputs.
- **Operator docs** in `docs/src/reference/` remain the runtime contract for how packaged apps are configured and run.

The practical portability claim should be:

> Vox aims for **build once per target set, run through a standardized OCI/runtime contract anywhere that contract exists**, not “ignore kernels and platforms entirely.”

This keeps scope disciplined, preserves cross-platform usefulness, and avoids pushing Vox toward a large OS-abstraction god object.

## Follow-on documents

This research now has three follow-on artifacts:

- [Vox Docker-backed portability implementation plan 2026](vox-docker-dotvox-portability-implementation-plan-2026.md)
- [ADR 015: Vox Docker/OCI portability SSOT](../adr/015-vox-docker-oci-portability-ssot.md)
- [Vox portability SSOT](../reference/vox-portability-ssot.md)

## Design intent

The design intent behind this direction is not merely “support Docker.”

The deeper goal is to choose a portability boundary that:

- is already widely implemented across Linux, macOS developer environments, Windows developer environments, CI, and cloud runtimes,
- gives Vox a reproducible packaging format,
- hides most host-specific deployment differences behind a stable operator interface,
- works with the existing package-manager and deployment work already in-tree,
- and lets Vox focus on language, package, and runtime semantics rather than raw host provisioning.

In that framing, Docker/OCI is not a side feature. It is the most realistic boundary for cross-platform execution without taking on the entire host-OS problem.

## Method and evidence quality

- Repo audit focused on active portability, PM, deployment, and SSOT surfaces:
  - [crates/vox-pm/src/manifest.rs](../../../crates/vox-pm/src/manifest.rs)
  - [crates/vox-pm/src/package_kind.rs](../../../crates/vox-pm/src/package_kind.rs)
  - [crates/vox-container/src/lib.rs](../../../crates/vox-container/src/lib.rs)
  - [crates/vox-container/src/deploy_target.rs](../../../crates/vox-container/src/deploy_target.rs)
  - [crates/vox-install-policy/src/lib.rs](../../../crates/vox-install-policy/src/lib.rs)
  - [contracts/cli/command-registry.yaml](../../../contracts/cli/command-registry.yaml)
  - [docs/src/reference/deployment-compose.md](../reference/deployment-compose.md)
  - [docs/src/architecture/vox-cross-platform-runbook.md](vox-cross-platform-runbook.md)
  - [docs/src/architecture/vox-packaging-research-findings-2026.md](vox-packaging-research-findings-2026.md)
  - [docs/src/architecture/vox-packaging-implementation-blueprint.md](vox-packaging-implementation-blueprint.md)
  - [docs/src/explanation/zig-inspired-deployment.md](../explanation/zig-inspired-deployment.md)
- External benchmark pass: 22 web searches, weighted toward canonical specs and project-maintainer documentation.
- Source weighting:
  - Tier A: official specs and vendor docs.
  - Tier B: maintainer or standards-adjacent docs.
  - Tier C: ecosystem analysis for tradeoff framing only.

## Why Docker/OCI is the right portability boundary

### What problem it solves well

Docker/OCI gives Vox a common packaging and execution contract for deployed applications:

- dependency payloads travel with the app,
- runtime expectations are explicit,
- distribution works through standard registries,
- image metadata, attestation, and signing have mature tooling,
- multi-architecture images can be published behind one logical tag,
- and CI/local/prod can share one artifact model.

This is a better fit than trying to make the language directly abstract every OS deployment detail.

### What problem it does not solve

Containers do **not** erase all platform differences:

- containers share the host kernel,
- Linux containers are not the same thing as Windows containers,
- architecture mismatches still matter unless images are published as multi-arch,
- bind mounts, file watching, and local networking differ across Docker Desktop, Linux Docker, and Podman,
- and operator-managed secrets/config still need explicit policy.

So the portability promise must be disciplined:

- **portable artifact contract**: yes,
- **portable kernel semantics**: no,
- **portable developer workflow with documented caveats**: yes,
- **zero-runtime-assumption magic**: no.

### Why not make WASI the main answer

WASI/Wasmtime remains useful for script isolation and some narrow portability lanes, and the current docs already treat it that way. But for full deployed `.vox` applications, the container ecosystem is far more mature today in:

- networking,
- multi-service composition,
- registry distribution,
- operator familiarity,
- security scanning,
- provenance tooling,
- and deployment-controller integration.

WASI should remain a complementary lane, not the primary app-deployment portability story.

## Current-state architecture map

### Project contract already exists

`vox-pm` already exposes the strongest project-level contract candidate:

- `Vox.toml` in [crates/vox-pm/src/manifest.rs](../../../crates/vox-pm/src/manifest.rs)
- deployment intent through `[deploy]`
- package/artifact typing via [crates/vox-pm/src/package_kind.rs](../../../crates/vox-pm/src/package_kind.rs)

Important current signal:

- `Vox.toml` already models `container`, `bare-metal`, `compose`, `kubernetes`, and `coolify` deployment intent.
- `PackageKind` already treats VoxPM as one manager over multiple artifact classes (`library`, `application`, `skill`, `agent`, `workflow`, `snippet`, `component`).

This is the right foundation for a future “universe” concept. The repo does not need a separate top-level portability schema to start solving this.

### Deployment execution engine already exists

`vox-container` is already the correct implementation seam:

- [crates/vox-container/src/lib.rs](../../../crates/vox-container/src/lib.rs) exposes a unified `ContainerRuntime` abstraction over Docker and Podman.
- [crates/vox-container/src/deploy_target.rs](../../../crates/vox-container/src/deploy_target.rs) already models `DeployTarget::{Container,BareMetal,Compose,Kubernetes}`.

That is a strong sign that Vox should **compose around this crate** rather than inventing a monolithic “portability manager.”

### Operator-facing deployment docs already exist

The runtime/deploy contract already has real documentation anchors:

- [docs/src/reference/deployment-compose.md](../reference/deployment-compose.md)
- [docs/src/architecture/vox-cross-platform-runbook.md](vox-cross-platform-runbook.md)
- [docs/src/explanation/zig-inspired-deployment.md](../explanation/zig-inspired-deployment.md)

These pages already present Docker/Compose and target selection as the operator-facing model. The research direction should converge docs and code around that model, not replace it.

### Packaging research already identified the missing SSOT

[docs/src/architecture/vox-packaging-research-findings-2026.md](vox-packaging-research-findings-2026.md) already identifies the unresolved contract across:

- `Vox.toml`,
- `vox.lock`,
- `.vox_modules`,
- and cache/CAS boundaries.

That is the main missing piece for portability as well. Portability is not blocked by lack of ideas; it is blocked by lack of one enforced contract across package resolution, materialization, and deploy packaging.

### Toolchain distribution already has an SSOT pattern

[crates/vox-install-policy/src/lib.rs](../../../crates/vox-install-policy/src/lib.rs) is a good model for how Vox handles a narrower SSOT today:

- supported release targets,
- source-install policy,
- release owner/repo,
- sidecar naming,
- and alignment with release/build docs.

This is useful because it shows a pattern Vox can copy:

- one Rust authority,
- one human-facing contract,
- CI parity enforcement.

### CLI portability surface is not fully converged

[contracts/cli/command-registry.yaml](../../../contracts/cli/command-registry.yaml) is the machine-readable command SSOT, but it currently exposes PM verbs without a fully converged deploy/portability contract row set.

That does not mean a new system is needed. It means the portability story is partly modeled in code/docs and not yet fully surfaced through the same contract discipline as the packaging work.

## Recommended single source of truth model

### Core recommendation

Vox should use a **layered SSOT**, not a single mega-file:

| Layer | Authority | Responsibility |
| --- | --- | --- |
| Project desired state | `Vox.toml` | package intent, package kind, deploy intent, operator-declared settings |
| Project resolved state | `vox.lock` | exact dependency graph, digests/checksums, locked build inputs |
| Materialization and fetch | `vox-pm` | resolve, fetch, cache/CAS, offline/locked/frozen enforcement |
| Runtime/deploy execution | `vox-container` | build image, tag/push, compose/systemd/k8s emission and execution |
| Toolchain distribution | `vox-install-policy` | how `vox` itself ships across host triples |
| Surfaced command contract | `contracts/cli/command-registry.yaml` | user-visible verbs and CI compliance |
| Operator runtime contract | `docs/src/reference/` | env vars, compose/deploy behavior, runtime caveats |

This is the right kind of SSOT for the repo: **one authority per concern, with clear ownership boundaries**.

### Why not one giant portability object

Vox should avoid creating a central object that tries to own:

- manifest parsing,
- lockfile semantics,
- artifact fetching,
- image creation,
- compose generation,
- runtime detection,
- secret injection,
- registry publication,
- and toolchain install policy

all in one place.

That would become a portability god object and would likely duplicate logic already living in `vox-pm`, `vox-container`, `vox-config`, docs SSOTs, and CLI compliance.

Instead, the future implementation should keep the contract split and wire those surfaces together through explicit interfaces.

### Practical SSOT flow

```mermaid
flowchart LR
    voxSource[".vox project"] --> voxManifest["Vox.toml [deploy]"]
    voxManifest --> voxLock["vox.lock"]
    voxLock --> resolvedState["Resolved package graph"]
    resolvedState --> voxPm["vox-pm fetch/materialize"]
    voxPm --> voxContainer["vox-container packaging/deploy"]
    voxContainer --> ociImage["OCI image or OCI artifact"]
    ociImage --> runtimeSurface["Docker or Podman runtime"]
    runtimeSurface --> targetHost["Target host or platform"]
```

## Best practices the research supports

### 1. Treat OCI as the deployable artifact format

Vox should prefer OCI images as the default deployable output for application portability.

Where multi-service deployment is the right abstraction, Vox should evaluate publishing generated Compose bundles as OCI artifacts rather than inventing a separate bespoke distribution wrapper.

### 2. Make multi-arch publication a first-class portability rule

If Vox says “run this on common systems,” the published artifact strategy should assume at least:

- `linux/amd64`
- `linux/arm64`

for deployable application images, with more targets added where product value is clear.

Single-arch images are a compatibility foot-gun masquerading as portability.

### 3. Bind deployment to the lockfile

`vox.lock` should become mandatory input for reproducible packaging lanes:

- local locked builds,
- CI image builds,
- release promotion,
- and deployment packaging.

If container packaging is not lock-aware, portability becomes “works on my registry today,” not “reproducible deployment.”

### 4. Pin base images and publish immutable outputs

Best practice is to:

- pin base images by digest,
- pin deploy inputs by lock/checksum,
- sign or attest immutable digests,
- and promote digests instead of mutable tags when policy requires strong reproducibility.

### 5. Generate SBOM and provenance during build

BuildKit-native SBOM and provenance support means portability artifacts can also be auditable artifacts.

For Vox, this should be part of the deploy contract, especially for:

- CI promotion,
- enterprise usage,
- and reproducibility claims.

### 6. Use OCI metadata consistently

Images and related artifacts should carry standardized metadata for:

- source repository,
- revision,
- version,
- documentation URL,
- vendor,
- license,
- and base-image ancestry.

This is low-cost and makes later tooling, debugging, and policy verification substantially easier.

### 7. Keep config out of code and secrets out of images

The Twelve-Factor guidance remains the right baseline:

- config that varies per deploy should not live in code,
- environment variables remain the interoperable default for non-secret deploy config,
- secrets should not be baked into images,
- and secret resolution should align with existing Clavis policy rather than bypass it.

### 8. Support Docker first, keep Podman as a compatibility requirement

Because `vox-container` already supports both runtimes, Vox should:

- document Docker/OCI as the primary portability story,
- keep Podman compatibility for rootless Linux and operator preference,
- and treat runtime detection as an execution concern, not the top-level project contract.

### 9. Preserve clear boundaries between project portability and tool portability

There are two different portability stories:

- how the **`vox` toolchain** runs on supported host triples,
- how a **user’s `.vox` application** is packaged and deployed.

These should stay connected but not conflated.

`vox-install-policy` is the SSOT for the first problem. `Vox.toml` + `vox.lock` + `vox-container` should be the SSOT stack for the second.

## Non-goals and caveats

The research supports explicitly **not** promising the following:

- native, deep OS-specific packaging support for every target as a first-class Vox responsibility,
- container-free full portability across all deploy shapes,
- equivalence between Linux, macOS, and Windows runtime/kernel behavior,
- hidden secret management inside images,
- or a claim that WASI replaces the container deployment story.

Important caveats to document in future normative docs:

- Docker Desktop on macOS/Windows is still a Linux VM-backed experience for Linux containers.
- File watching, volume mounts, permissions, and localhost semantics differ across runtimes.
- Windows container support is a separate concern from Linux multi-arch support.
- Compose-as-OCI has real limitations around bind mounts, local includes, and build-only services.

## Current repo gaps

### Gap 1: deploy intent exists, but the full contract is not yet enforced

`Vox.toml [deploy]` exists, but the deploy package/build lifecycle is not yet consistently enforced from:

- manifest,
- to lock,
- to fetch/materialize,
- to image build,
- to publication.

### Gap 2: docs imply a unified deploy story more strongly than the CLI contract does

The docs already speak in a unified `vox deploy` voice, but the machine-readable command SSOT and some code paths have not fully converged around that public contract.

### Gap 3: package “universe” exists conceptually, but not yet as a deployment-aware contract

`PackageKind` and `vox-pm` strongly suggest one package universe, but the link between:

- package identity,
- deployable application packaging,
- OCI publication,
- and runtime portability metadata

is not yet described as one coherent system contract.

### Gap 4: container reproducibility is strategic, but not yet an always-on requirement

The packaging research already points at locked/frozen/container reproducibility as a target. This portability direction makes that requirement non-optional.

### Gap 5: operator docs and implementation boundaries need one normative handoff

The repo has the right raw pieces, but it still needs a clearer handoff between:

- research/design intent,
- future normative operator docs,
- and eventual implementation-plan tasks.

## Recommended route forward

### Route 1: declare the architecture and boundary now

Adopt the following architectural statement:

> Vox application portability is primarily achieved through a lock-bound Docker/OCI packaging contract, surfaced by `Vox.toml` and executed by `vox-container`, rather than by deep host-specific runtime support in the language core.

This should become the working assumption for future implementation planning.

### Route 2: make `Vox.toml [deploy]` the declarative entrypoint

Continue extending `[deploy]` as the project-author intent surface rather than inventing parallel deploy metadata files.

Short-term implication:

- keep adding deploy fields there,
- validate them consistently,
- and ensure operator-facing docs refer back to that one entrypoint.

### Route 3: make `vox.lock` deployment-relevant, not only package-relevant

The future implementation plan should explicitly define how `vox.lock` participates in:

- image construction,
- offline/frozen packaging,
- cache materialization,
- artifact verification,
- and reproducible deployment.

### Route 4: let `vox-container` stay focused on runtime mechanics

`vox-container` should own:

- runtime detection,
- image generation/build invocation,
- compose/systemd/k8s emission,
- and target execution.

It should **not** absorb PM resolution policy or become the single owner of every portability concern.

### Route 5: use OCI registries as the distribution substrate

The likely best medium-term direction is:

- package dependencies and metadata remain under `vox-pm` concepts,
- deployable apps publish OCI images,
- multi-service app bundles can optionally publish OCI artifacts,
- and future provenance/signature data lives alongside those artifacts in the registry ecosystem.

This reuses mature auth, storage, CDN, and policy tooling rather than building a custom artifact server for deployment semantics from scratch.

### Route 6: formalize portability best practices in CI

The future implementation plan should likely turn these into explicit checks:

- base-image digest pinning,
- `vox.lock` required in locked deploy lanes,
- multi-arch manifest publication,
- SBOM generation,
- provenance attestations,
- and image metadata/annotation completeness.

### Route 7: split normative docs from research once decisions harden

This research doc should remain the analytical record.

Once decisions are accepted, the repo should likely add:

- a reference-grade portability/deployment SSOT page under `docs/src/reference/`,
- and possibly an ADR for the architectural decision itself.

## Guidance for a future implementation plan

The later implementation plan should answer these concrete questions:

1. What exact fields must `vox.lock` carry to make deployment reproducible?
2. How should `vox deploy` be surfaced and validated in the CLI contract registry?
3. Which OCI labels/annotations are mandatory for Vox-built artifacts?
4. What CI gates are required versus advisory?
5. Which deployment outputs are supported in phase 1:
   - OCI image only
   - Compose emission
   - OCI artifact bundle for Compose
   - bare-metal/systemd bridge
   - Kubernetes emission
6. What is the minimum supported multi-arch matrix?
7. How should secrets/config be injected across local, CI, and hosted runtimes without bypassing Clavis or env-var SSOTs?

## Recommended position on the package-manager “universe”

The cleanest direction visible from the current repo is:

- **one package universe** for Vox artifacts under `vox-pm`,
- **one project contract** in `Vox.toml` + `vox.lock`,
- **one deploy execution engine** in `vox-container`,
- **one operator-facing deployment contract** in docs/reference,
- and **one distribution substrate family** in OCI registries for deployable outputs.

That does not mean every artifact must become an OCI image.

It means Vox should stop treating packaging, deployment, and portability as unrelated systems. They are one chain with different artifact layers and different owners.

## Bibliography (core)

### Tier A

- Docker Docs: [Multi-platform builds](https://docs.docker.com/build/building/multi-platform/)
- Docker Docs: [Package and deploy Docker Compose applications as OCI artifacts](https://docs.docker.com/compose/how-tos/oci-artifact)
- Docker Docs: [SBOM attestations](https://docs.docker.com/build/metadata/attestations/sbom)
- OCI spec: [Image annotations](https://specs.opencontainers.org/image-spec/annotations/?v=v1.1.0)
- Twelve-Factor App: [Config](https://12factor.net/config)
- GitHub Docs: [Artifact attestations and SLSA v1 Build Level 3](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations/using-artifact-attestations-and-reusable-workflows-to-achieve-slsa-v1-build-level-3)
- SLSA: [Get started](https://slsa.dev/how-to/get-started)

### Tier B

- Docker Docs: [Build annotations](https://docs.docker.com/build/metadata/annotations/)
- Docker Docs: [Compose publish reference](https://docs.docker.com/reference/cli/docker/compose/publish/)
- Sigstore: [Signing containers with Cosign](https://docs.sigstore.dev/cosign/signing/signing_with_containers/)
- ORAS: [Pushing and pulling OCI artifacts](https://oras.land/docs/how_to_guides/pushing_and_pulling)
- Podman Docs: [podman-systemd.unit / Quadlet](https://docs.podman.io/en/v5.0.0/markdown/podman-systemd.unit.5.html)

### Tier C

- Ecosystem comparisons and tradeoff analyses were used only to frame operational caveats around rootless runtimes, multi-arch workflows, and base-image choices.


