---
title: "CI alternatives and local Docker-based mirroring"
description: "Research on running GitHub Actions locally in Docker, alternatives to GitHub Actions (act, Earthly, Dagger, Forgejo, Gitea, Woodpecker, GitLab CI, BuildJet/Blacksmith/RunsOn), and how each integrates with the existing vox ci pre-push gate."
category: "CI & Quality"
training_eligible: true
schema_type: "TechArticle"
---

# CI alternatives and local Docker-based mirroring

> **Update (2026-06-03):** The parallel GitLab CI mirror (`.gitlab-ci.yml`) has been **retired and deleted** — GitLab CI is no longer a supported target. The notes below that described it as "already mirrored" / "a safety net" are kept for historical context but no longer reflect the repo; GitHub Actions (`.github/workflows/`) is the sole CI surface.

Research output. No workflow YAML or runner topology is changed by this
document — it captures findings so we can decide how (or whether) to invest.

## TL;DR

1. **Free-tier minutes are not our bottleneck.** `ci.yml` and the heavy ML lanes
   already run on **self-hosted Linux** ([runner-contract](runner-contract.md)).
   The only GitHub-hosted minutes we still pay for are documented exceptions:
   `docs-deploy`, `docs-quality`, `link_checker`, `release-binaries`
   (`windows-latest` / `macos-latest`), `vox-vscode-extension`, and two
   `ubuntu-latest` smokes inside `ci.yml`
   ([github-hosted-exceptions](github-hosted-exceptions.md)).
2. **Local-Docker-as-gate already exists in skeleton form.** `vox ci pre-push`
   ([local-ci-pre-push](../contributors/local-ci-pre-push.md)) is the supported
   entry point. The fastest, lowest-risk improvement is to **graft `act` (or
   Earthly) onto that hook**, not to introduce a new CI engine.
3. **Replacing GitHub Actions wholesale is not warranted.** GitLab CI was
   previously kept as a parallel mirror but was **removed on 2026-06-03** — the
   parallel pipeline was not worth maintaining. Forgejo Actions and Gitea
   Actions are GH-Actions-compatible drop-ins worth tracking but offer no
   decisive win over our current self-hosted fleet.
4. **The biggest unrealised speedup is Rust build caching**, not the CI
   provider: `sccache` + `mold` + a shared `target/` cache delivers ~2–4×
   wall-clock improvements in our matrix; provider choice is downstream of
   that.

## Current state (concise)

| Layer | What we use today | Notes |
|---|---|---|
| Default runner | `[self-hosted, linux, x64]` | Free in minute terms; bottleneck is wall-clock + capacity. |
| Docker / Buildx jobs | `[self-hosted, linux, x64, docker]` | Used by `mesh-compose-config`, `docker-vox-image-smoke`, `all-features-matrix`. |
| Browser / Playwright | `[self-hosted, linux, x64, browser]` | Chromium pool. |
| GH-hosted exceptions | `ubuntu-latest`, `windows-latest`, `macos-latest` | 7 workflow surfaces; documented. |
| Local mirror | `vox ci pre-push` | Quick / default / full modes (~30 s / 2–4 min / 10–25 min). |

The architecture already separates "guard logic" (Rust binaries under
`crates/vox-cli/src/commands/ci/`) from "workflow YAML" (`.github/workflows/`).
That makes provider replacement cheap: any of the alternatives below can shell
into the same `vox ci …` commands.

## Local Docker-based gating

### Option A — `act` (nektos/act)

`act` reads `.github/workflows/*.yml` and runs each job inside a Docker image
that mimics the GitHub-hosted runner image. It is the closest-to-native local
substitute for `ubuntu-latest`.

**Fit for this repo:**

- Strong fit for the GH-hosted exceptions (`docs-quality.yml`,
  `link_checker.yml`, `vox-vscode-extension`, `visualizer-ingest-smoke`,
  `web-vite-build-smoke`). These are small, Linux-only, and well-suited to a
  catalog image.
- Weak fit for self-hosted-labelled jobs. `act` matches `runs-on: ubuntu-latest`
  but not `[self-hosted, linux, x64, docker]` without `--platform` overrides.
  The composite labels (`docker`, `browser`, `gpu`) would need explicit mapping
  flags in `.actrc`.
- Cannot reproduce GPU, CUDA `nvcc`, or real Chromium-with-display lanes
  faithfully — those stay on the self-hosted fleet.

**Integration shape:**

1. Add an `act-pre-push` mode to `vox ci pre-push` that runs a subset of
   workflows by `--workflows` filter against the local Docker daemon.
2. Cache layers: bind-mount `~/.cache/act`, `~/.cargo`, and a project-local
   `target-act/` so `cargo build` is not paid per-push.
3. Default to **opt-in**, not on by default — `vox ci pre-push --act` — to
   avoid surprising contributors without Docker.

**Cost model:** zero $; ~3–8 min added to a typical pre-push for the
exception lanes. Catches GH-hosted-only failures (e.g. Node/pnpm version
mismatch) before they burn minutes.

**Caveats:**

- `act`'s default catalog image lags GitHub's image ~weeks. We have already
  pinned action major versions (`actions/checkout@v6`, `actions/cache@v5`)
  and require runner v2.327.1+ on self-hosted; `act` images need the same
  spot-check.
- Secrets handling: `act` reads `.secrets` files; never commit them.
  `.secrets` is in [`.gitignore`](../../../.gitignore).
- Network egress in `act` containers is unrestricted by default; tighten
  with `--network none` for guard-only lanes.

**Windows support.** `act` runs on Windows via WinGet, Scoop, Chocolatey, or
the `gh act` extension; the supported daemon is the **WSL2-native Docker Engine**
(`docker-ce` installed inside the WSL2 distro — Docker Desktop is not used on this
host; see [runner-autoscaling.md](runner-autoscaling.md)). No special invocation is
needed: `act` picks up the active `docker-wsl` SSH context like any other Docker
client. Install + troubleshooting tables: [local-ci-pre-push.md
§Installing `act`](../contributors/local-ci-pre-push.md#installing-act).
Both `--bind` (cargo cache mount) and `--artifact-server-path` may need
Windows-specific overrides; documented in the same section.

**macOS support.** `act` installs via Homebrew (`brew install act`). As on Windows,
**Docker Desktop is not used** — the supported daemon is [colima](https://github.com/abiosoft/colima)
(`brew install colima`), which exposes a normal Docker socket that `act` picks up
through its own `colima` context with no special invocation.

Size the VM for the host and enable Rosetta — `.actrc` pins
`--container-architecture linux/amd64` so local runs match the amd64
GitHub-hosted runners, and without Rosetta those containers fall back to slow QEMU
emulation:

```bash
colima start --vm-type vz --vz-rosetta --cpu 12 --memory 48 --disk 100
```

colima defaults to 2 CPU / 2 GiB, which is far too small for a Rust workspace this
size; scale the flags to the machine (leave the host several cores and enough RAM
for a native `cargo build` alongside). Verify with `colima list` and
`docker info --format '{{.NCPU}} {{.MemTotal}}'`, and confirm emulation works with
`docker run --rm --platform linux/amd64 alpine uname -m` (expect `x86_64`).

Apple Silicon caveat: the pinned `catthehacker/ubuntu:act-*` images publish
`linux/arm64/v8` as well as `linux/amd64`. Dropping the architecture pin makes act
silently use the arm64 variant — faster, but it exercises a different toolchain
than CI, the same class of false green as mapping `windows-latest` onto a Linux
image.

### Option B — Earthly

Earthly compiles `Earthfile`s into Buildkit pipelines that run in Docker.
Native shared-cache (`--push --cache-from`), parallelism, and reproducible
builds. Mature Rust support (`earthly/lib/rust`).

**Fit:**

- Strong for **release artifact builds** and **multi-target matrix** lanes
  (`all-features-matrix`, the per-crate `cargo check --all-features` matrix,
  the Docker image smoke).
- Weak as a wholesale GH Actions replacement: it does not consume
  `.github/workflows/*.yml`. We would maintain Earthfiles in parallel.

**Integration shape:** keep GitHub Actions as the trigger surface; have CI
jobs `earthly --ci +all-features` for the heavy matrices. Same Earthfile runs
locally with one command. Buildkit cache can be pushed to a registry
(GitHub Container Registry already authorised in our `permissions:
packages: write`).

**Cost model:** zero $ if cache is local; modest GHCR storage if cache is
shared between contributors.

### Option C — Dagger

Dagger is "CI as code" — pipelines authored in Go/Python/TypeScript that run
on a Buildkit engine inside Docker. More flexible than Earthly, more code
than YAML.

**Fit:** overkill for our current shape. We already have the abstraction
boundary in `vox ci`; introducing Dagger duplicates that. Re-evaluate only if
we want pipeline introspection or branching dataflow.

### Option D — Buildkit + scripts (lowest-ceremony)

For the narrow goal "let me run the same Linux build locally before push,"
a single `Dockerfile.ci` + `vox ci docker-mirror` would cover ~80 % of the
need without adopting a new tool. Re-uses our existing self-hosted runner
image if we publish it to GHCR.

## Alternatives to GitHub Actions (provider-level)

Ranked by fit for this codebase:

### 1. Stay on GitHub Actions + self-hosted (recommended)

Already done. The architecture's strength is **provider-agnostic guard
logic** in `vox ci`. As long as that holds, switching providers is a YAML
rewrite, not a logic rewrite. Investing in `vox ci` parity is higher ROI than
investing in a new provider.

### 2. Forgejo Actions / Gitea Actions

Both are open-source forge stacks that re-implement the GitHub Actions
runner protocol. `actions/checkout`, `actions/cache`, and `actions/setup-*`
all work. Self-hostable; no per-minute fee.

**When to consider:** if we ever want to leave GitHub for sovereignty or
cost reasons, this is the path with the lowest migration cost — workflows
move almost verbatim. Tracked dependency: third-party actions in our
workflows are `dtolnay/rust-toolchain`, `taiki-e/install-action`,
`pnpm/action-setup`, `actions/setup-node`, `actions/cache`,
`actions/checkout`, `actions/upload-artifact` — all JS or container actions
that run on Forgejo/Gitea Actions today.

**When NOT to consider:** purely as a free-tier escape; we already escaped
via self-hosted.

### 3. Woodpecker CI

Drone fork. YAML pipelines, Docker-native, OSS. Lighter than GitLab CI,
no Actions compatibility. Migration cost = full YAML rewrite.

**Fit:** none right now. Worth knowing it exists if we ever want a
container-first OSS server we control.

### 4. GitLab CI

**Retired (2026-06-03).** Vox previously maintained a `.gitlab-ci.yml` mirror
for job parity; it was deleted because keeping a parallel pipeline alongside
GitHub Actions was not worth the upkeep, and GitLab CI is no longer a supported
target here. It remains a viable alternative *provider* in the abstract should
we ever migrate, but re-adopting it would now be a full from-scratch port, not a
re-sync — not recommended.

### 5. Faster GitHub-Actions-compatible runner clouds

For the GH-hosted exception workflows where `ubuntu-latest` minutes burn,
drop-in faster runners exist:

| Provider | Pricing model | Notes |
|---|---|---|
| BuildJet | per-minute, ~50 % of GH price for 2× CPU | Drop-in `runs-on: buildjet-4vcpu-ubuntu-2204`. |
| Blacksmith | per-minute, similar tier | Cache acceleration for Rust. |
| RunsOn | flat $/mo, AWS-backed | Best for high-volume; we don't qualify. |
| Namespace.so | per-minute, fast cold start | Good for matrix fan-out. |

**Verdict:** not worth integrating today. Our GH-hosted minutes are
low-volume (docs/release/vscode), and adding a vendor adds an audit
surface for something measured in dollars per month.

### 6. CircleCI / Buildkite / Travis

Not recommended. None offer a meaningful delta over our current setup, and
all introduce a second source-of-truth for pipeline definitions.

## Recommendations (priority order)

These are sequenced by "smallest diff with biggest signal" first.

1. **Adopt `act` as an opt-in pre-push lane** for the GH-hosted exception
   workflows only. Add `vox ci pre-push --act` that runs `docs-quality`,
   `link_checker`, `visualizer-ingest-smoke`, `web-vite-build-smoke`, and
   `vox-vscode-extension` against `act` with cached `~/.cache/act` +
   bind-mounted `~/.cargo`. Catches the failures that today only surface
   after push.
2. **Publish the self-hosted runner image to GHCR** so contributors can
   `docker pull ghcr.io/<org>/vox-ci-runner` and reproduce the heavy
   self-hosted lanes locally. This is the lever for "run the real CI
   locally," not `act`.
3. **Audit `act` image drift quarterly** if we adopt it — pin the catalog
   image SHA in `.actrc` and bump alongside our `actions/*` major-version
   bumps.
4. **Defer Earthly / Dagger.** Re-evaluate if `all-features-matrix` wall-clock
   becomes a sustained pain point — Earthly's shared Buildkit cache is the
   strongest mitigation.
5. **Track Forgejo Actions** as a contingency only. No work needed today;
   a `gh-actions` ↔ `forgejo-actions` parity check could be added to
   `vox ci command-compliance` later if the contingency becomes real.
6. **Do not adopt CircleCI / Buildkite / Travis / Drone / Woodpecker.**

## Non-goals

- Replacing the self-hosted runner fleet — out of scope; orthogonal to
  provider choice.
- Auto-blocking commits on `act` results — pre-push is the right surface
  per `AGENTS.md`; pre-commit is too noisy for this codebase's build
  times.

## Open questions for the next iteration

- Does `act` correctly handle `--shell: pwsh` steps (we have a few in
  `ci.yml`)? Needs a one-shot smoke run before any contributor-facing
  rollout.
- Can the self-hosted image be sliced into thinner variants (basic /
  docker / browser) so GHCR pulls are cheap? Today they're one runner per
  capacity pool tag.
- Is `cargo-llvm-cov` reliable inside `act`'s nested-Docker image without
  privileged mode? If not, the `tests` lane stays self-hosted-only —
  acceptable.

## See also

- [Runner contract](runner-contract.md)
- [GitHub-hosted exceptions](github-hosted-exceptions.md)
- [Workflow enumeration](workflow-enumeration.md)
- [Local CI parity (pre-push)](../contributors/local-ci-pre-push.md)
- [`AGENTS.md` §VoxScript-First Glue Code](../../../AGENTS.md)
