---
title: "Vox portability SSOT"
description: "Normative portability contract for Docker/OCI-backed `.vox` deployment, source-of-truth boundaries, and conformance expectations."
category: "Language Reference"
last_updated: "2026-05-05"
training_eligible: true

schema_type: "TechArticle"
---

This page defines the normative portability contract for deployed `.vox` applications.

**Sibling contract (end-user app installers):** shipping desktop/mobile **installable** artifacts (MSI/DMG/APK/IPA, etc.) is documented in [Vox application packaging SSOT (2026)](../architecture/vox-application-packaging-ssot-2026.md). That lane complements this page’s Docker/OCI deploy focus and does not relax server-portability requirements here.

For background and rationale, see:

- [ADR 015](../adr/015-vox-docker-oci-portability-ssot.md)
- [Vox Docker-backed portability research 2026](../archive/research-2026-q1/vox-docker-dotvox-portability-research-2026.md)
- [Vox Docker-backed portability implementation plan 2026](../archive/research-2026-q1/vox-docker-dotvox-portability-implementation-plan-2026.md)

## Portability contract

Vox application portability means:

- a `.vox` project can declare deploy intent once,
- the resolved project state can be packaged into a standardized deployable artifact contract,
- and that artifact can be executed on supported runtime surfaces with documented caveats.

Vox portability does **not** guarantee:

- identical kernel behavior across host operating systems,
- transparent equivalence between Linux and Windows containers,
- support for every host/runtime combination,
- or secret management embedded inside application images.

## Canonical source-of-truth boundaries

| Concern | Canonical authority |
| --- | --- |
| Project desired state | `Vox.toml` |
| Project resolved state | `vox.lock` |
| Dependency resolution / fetch / cache / materialization | `vox-package` |
| Runtime-specific packaging and deployment | `vox-container` |
| User-visible CLI contract | `contracts/cli/command-registry.yaml` |
| Operator/runtime reference policy | `docs/src/reference/` |
| Toolchain release portability for `vox` | `crates/vox-install-policy/src/lib.rs` |

## Required invariants

### Desired-state and resolved-state

- `Vox.toml` **must** remain the project desired-state contract.
- `vox.lock` **must** remain the project resolved-state contract.
- Deploy packaging **must not** rely on undocumented implicit host state once a lock-bound lane is in effect.

### Packaging and artifact policy

- Portable app deployment **must** use Docker/OCI-backed packaging as the primary boundary.
- Deployable images **should** be published as multi-architecture artifacts where portability claims require it.
- Base images **should** be pinned by digest in reproducibility-sensitive lanes.
- Promoted deploy artifacts **should** carry OCI metadata for source, revision, version, documentation, and license where supported.

### Supply-chain and verification

- Release-grade portability lanes **should** generate SBOM data.
- Release-grade portability lanes **should** generate provenance attestations.
- Signing policy **should** be applied to promoted immutable artifacts, especially where registry or deployment policy depends on verification.

### Config and secrets

- Per-deploy configuration **must not** be hardcoded into application code.
- Secrets **must not** be baked into committed images.
- Deploy configuration **should** use environment-variable conventions documented in [Environment variables (SSOT)](env-vars.md).
- Secret resolution **must** stay aligned with [Secrets SSOT](secrets-ssot.md).

## Runtime support statement

- Docker is the primary documented portability abstraction for deployed `.vox` applications.
- Podman compatibility is required where `vox-container` advertises runtime parity, especially for rootless/operator workflows.
- Runtime detection is an execution concern, not a replacement for project-level deploy intent.
- WASI/Wasmtime is a complementary execution/isolation lane and not the primary deployed-app portability boundary.
- Stock-phone execution of the full Vox CLI/toolchain is not a portability requirement for this contract.
- Mobile support is primarily browser-app portability plus remote control of a non-phone Vox host.

## Portable backend artifact lane

For backend portability workstreams, use a single release-grade lane with explicit provenance:

- Build artifacts from lock-bound project state (`Vox.toml` + `vox.lock`).
- Publish OCI images with source/revision metadata labels.
- Generate and retain SBOM + provenance attestations for promoted builds.
- Apply signing policy before promotion to runtime environments.
- Avoid embedding secrets in images; runtime secret resolution remains Clavis-managed.

### Repo-local artifact markers (`vox deploy`)

When **`VOX_BACKEND_ARTIFACT_SBOM_REQUIRED`** or **`VOX_BACKEND_ARTIFACT_SIGNING_REQUIRED`** are truthy ([environment SSOT](env-vars.md)), **`vox deploy`** requires marker files under **`<repo_root>/.vox/backend-artifact/`** (resolved via **`vox_config::paths::repo_backend_artifact_dir`**) before a non–dry-run deploy on OCI-facing targets (`container`, `compose`, `kubernetes`, `fly`, `coolify`):

- **SBOM** (at least one): `sbom.json`, `sbom.spdx.json`, or `sbom.cyclonedx.json`
- **Signing** (at least one): `signing.attestation.json` or `artifact.sig`

## Compatibility caveats

- Containers share the host kernel. Portability claims apply to the artifact/runtime contract, not to kernel identity.
- Linux-container portability and Windows-container portability are separate concerns.
- Architecture mismatches remain relevant unless multi-arch publication is in place.
- Docker Desktop on macOS and Windows introduces VM-backed behavior differences for Linux containers.
- Volume mounts, file watching, permissions, and local networking can differ across Docker, Docker Desktop, and Podman.
- Compose-as-OCI workflows have limitations around bind mounts, local includes, and build-only services.

## Conformance checklist

Use this checklist when defining or validating portability-sensitive lanes:

- [ ] `Vox.toml` is the deploy-intent entrypoint; no parallel undeclared deploy schema is introduced.
- [ ] `vox.lock` role in deploy packaging is explicit.
- [ ] `vox-package` vs `vox-container` ownership is clear and not duplicated.
- [ ] Operator docs distinguish app portability from toolchain portability.
- [ ] Docker/OCI is the primary deploy portability boundary in docs and code comments.
- [ ] Podman compatibility claims are explicit and scoped.
- [ ] Multi-arch requirements are stated for the relevant publication lane.
- [ ] Digest-pinning expectations are stated for reproducibility-sensitive builds.
- [ ] SBOM/provenance/signing policy is stated for promoted artifacts.
- [ ] Secret/config behavior cites `env-vars.md` and `secrets-ssot.md`.
- [ ] CLI contract implications are consistent with `contracts/cli/command-registry.yaml`.

## Related operational references

- [Deployment: Docker, Compose, Coolify, CI (SSOT)](deployment-compose.md)
- [Cross-platform Vox — runbook](../archive/research-2026-q1/vox-cross-platform-runbook.md)
- [Environment variables (SSOT)](env-vars.md)
- [Secrets SSOT](secrets-ssot.md)
- [Command compliance](command-compliance.md)


