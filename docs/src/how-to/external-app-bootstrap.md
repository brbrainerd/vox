---
title: "Bootstrap a Vox application outside the monorepo"
description: "Install the toolchain, scaffold a project, and choose how to depend on platform crates like vox-db."
category: "How-To Guides"
status: "current"
---

# Bootstrap a Vox application outside the monorepo

This guide is for consumer-facing apps that live in **their own Git repository** (not under `vox-lang/vox`). It complements **`docs/src/tutorials/tut-getting-started.md`** and **`docs/src/reference/installation.md`**.

## 1. Install `vox`

- Prefer release binaries per **`docs/src/reference/installation.md`** (`scripts/install.ps1` / `scripts/install.sh`), or **`vox upgrade`** from an existing install.
- Until `vox` is on `PATH`, use the thin launchers **`scripts/windows/vox-dev.ps1`** / **`scripts/vox-dev.sh`** only when developing **inside** this repository.

## 2. Scaffold the project

From an empty directory:

```bash
vox init my-app --template mobile-pwa   # or web, api, chatbot
cd my-app
vox check src/main.vox
vox build src/main.vox -o dist
```

The **`mobile-pwa`** template emits **`import std.mobile`**, Capacitor config, PWA manifest, and **`package.json`** with **`@capacitor/push-notifications`** (required by generated **`mobile-utils.ts`** when targeting **`android`/`ios`**).

## 3. Choosing how to persist data (`vox-db`)

**Today, `vox-db` is built as part of the Vox workspace** (see **`crates/vox-db/Cargo.toml`** — workspace path crates). There is no small crates.io-only drop-in yet.

Pick one pattern:

| Pattern | When to use | Trade-off |
|--------|----------------|-----------|
| **A. Git submodule + workspace path** | You control both repos; want full `table` + Turso/libSQL today | You vendor/link the Vox tree; larger checkout |
| **B. HTTP API only** | App stores nothing in-process SQLite; backend runs in your deployment | Requires a server (not suitable for strict offline-only mobile clients) |
| **C. Wait for / contribute `vox-db-client` extraction** | You need a minimal published crate without `vox-compiler` in the graph | Not available yet; track **`docs/src/architecture/data-storage-ssot-2026.md`** |

For **strict local-only mobile** (no server), pattern **A** or a **native Capacitor SQLite plugin** in the app shell is typical until **`vox-db`** is consumable as a standalone library.

## 4. Package manager and locks

- Use **`pnpm`** for Node assets alongside Capacitor/Vite when **`package.json`** is present (**`AGENTS.md`**).
- Pin the **`vox`** toolchain version your CI installs (see **`docs/src/ci/binary-release-contract.md`**) so builds reproduce.

## 5. Automation in the app repo

Project automation must be **`.vox` scripts** executed with **`vox run`** (see **`AGENTS.md` § VoxScript-first**). CI should install **`vox`** first, then invoke those scripts—there is no chicken-and-egg once **`vox`** is installed.

## 6. Documentation split

- **This repository (`docs/src/`)** remains the SSOT for the **Vox language and platform**.
- **Your app repository** should own product READMEs, privacy statements, clinician export semantics, and app ADRs; link here instead of duplicating compiler SSOT.

## Related

- **`examples/oratio/codexAudioTranscribe.ts`** — HTTP Oratio client example (`Speech.transcribe` remains server-side on TS builds).
- **`std.mobile.transcribe_microphone()`** / **`Speech.transcribe_microphone()`** — on-device STT hook; requires a **`VoxSherpaTranscribe`** Capacitor plugin at runtime (see codegen **`mobile-utils.ts`**).
