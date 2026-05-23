---
title: "Single-file parent directories — triage list (2026)"
description: "Machine-generated list of tracked paths whose immediate parent directory contains exactly one file; use for idiomatic vs merge decisions."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: false
---

# Single-file parent directories (triage list)

Regenerate:

```powershell
git ls-files | ForEach-Object { Split-Path $_ -Parent } |
  Group-Object | Where-Object Count -eq 1 | Sort-Object Name |
  ForEach-Object { $_.Name }
```

**How to use:** Tag rows as *idiomatic* (Rust module, contract leaf, VS Code feature folder), *candidate merge*, or *generated*, then open a focused PR per merge cluster.

| Immediate parent (one tracked file) |
|-------------------------------------|
| `.claude` |
| `.vox\bin` |
| `.vscode` |
| `apps\editor\vox-vscode\snippets` |
| `apps\editor\vox-vscode\src\agents` |
| `apps\editor\vox-vscode\src\chat` |
| `apps\editor\vox-vscode\src\context` |
| `apps\editor\vox-vscode\src\gamify` |
| `apps\editor\vox-vscode\src\models` |
| `apps\editor\vox-vscode\src\speech` |
| `apps\editor\vox-vscode\src\vcs` |
| `apps\vox-mental-tracker\contracts\event-payloads` |
| `apps\vox-mental-tracker\docs\contributors` |
| `apps\vox-mental-tracker\docs\user` |
| `apps\vox-mental-tracker\plugins\vox-sherpa-transcribe\android` |
| `apps\vox-mental-tracker\plugins\vox-sherpa-transcribe\android\src\main` |
| `apps\vox-mental-tracker\plugins\vox-sherpa-transcribe\android\src\main\java\com\vox\plugins\voxsherpatranscribe` |
| `apps\vox-mental-tracker\plugins\vox-sherpa-transcribe\src` |
| `apps\vox-mental-tracker\public\icons` |
| `apps\vox-mental-tracker\src\pages` |
| `apps\vox-mental-tracker\tests\fixtures` |
| `apps\vox-mental-tracker\web-dist\assets` |
| `contracts\benchmarks` |
| `contracts\ci` |
| `contracts\dei` |
| `contracts\governance` |
| `contracts\hir` |
| `contracts\naming` |
| `contracts\populi` |
| `contracts\reports\scaling-audit\rollup` |
| `contracts\review` |
| `contracts\tokens` |
| `contracts\workflow` |
| `crates` |
| `crates\vox-actor-runtime\src\container` |
| `crates\vox-actor-runtime\src\storage` |
| `crates\vox-agentos-mutation` |
| `crates\vox-agentos-mutation\src` |
| `crates\vox-arch-check\src` |
| `crates\vox-arch-check\tests\fixtures\missing-desc` |
| `crates\vox-arch-check\tests\fixtures\missing-desc\src` |
| `crates\vox-bounded-fs` |
| `crates\vox-bounded-fs\src` |
| `crates\vox-bounded-fs\tests` |
| `crates\vox-build-meta` |
| `crates\vox-build-meta\src` |
| `crates\vox-capability-registry` |
| `crates\vox-capability-registry\tests` |
| `crates\vox-checksum-manifest` |
| `crates\vox-checksum-manifest\src` |
| `crates\vox-claim-extractor` |
| `crates\vox-claim-extractor\tests` |
| `crates\vox-cli-ci` |
| `crates\vox-cli-ci\src` |
| `crates\vox-cli-core` |
| `crates\vox-cli\src\bin` |
| `crates\vox-cli\src\commands\ci\run_body_helpers\matrix` |
| `crates\vox-cli\src\commands\extras\share` |
| `crates\vox-cli\src\commands\extras\snippet` |
| `crates\vox-cli\src\commands\migrate` |
| `crates\vox-cli\src\commands\runtime` |
| `crates\vox-cli\src\commands\visus` |
| `crates\vox-cli\tests\fixtures\migrate\after` |
| `crates\vox-cli\tests\fixtures\migrate\before` |
| `crates\vox-cli\tests\golden` |
| `crates\vox-cli\wix` |
| `crates\vox-codegen` |
| `crates\vox-codegen\src\web_ir\primitives` |
| `crates\vox-compiler\src\builtin\std` |
| `crates\vox-compiler\tests\fixtures` |
| `crates\vox-compiler\tests\fixtures\props` |
| `crates\vox-config` |
| `crates\vox-constrained-gen` |
| `crates\vox-container` |
| `crates\vox-corpus\src\mcp_meta` |
| `crates\vox-corpus\tests` |
| `crates\vox-crypto` |
| `crates\vox-dashboard\app\src` |
| `crates\vox-db-types` |
| `crates\vox-db-types\tests` |
| `crates\vox-db\examples` |
| `crates\vox-db\src\legacy` |
| `crates\vox-db\src\schema\spec` |
| `crates\vox-db\tests\common` |
| `crates\vox-deploy-codegen` |
| `crates\vox-doc-inventory` |
| `crates\vox-drift-check` |
| `crates\vox-drift-check\src\bin` |
| `crates\vox-drift-check\tests` |
| `crates\vox-eval` |
| `crates\vox-eval\src` |
| `crates\vox-exec-grammar` |
| `crates\vox-exec-grammar\tests` |
| `crates\vox-forge` |
| `crates\vox-git` |
| `crates\vox-grammar-export` |
| `crates\vox-grammar-export\tests` |
| `crates\vox-identity` |
| `crates\vox-inspect-bridge` |
| `crates\vox-install-policy` |
| `crates\vox-install-policy\src` |
| `crates\vox-integration-tests\playwright` |
| `crates\vox-integration-tests\src` |
| `crates\vox-jsonschema-util` |
| `crates\vox-jsonschema-util\examples` |
| `crates\vox-jsonschema-util\src` |
| `crates\vox-mcp-registry\src` |
| `crates\vox-mesh-types` |
| `crates\vox-ml-cli` |
| `crates\vox-nanopub` |
| `crates\vox-openai-sse` |
| `crates\vox-openai-sse\src` |
| `crates\vox-openai-wire` |
| `crates\vox-openclaw-runtime` |
| `crates\vox-oratio` |
| `crates\vox-orchestrator-core` |
| `crates\vox-orchestrator-core\src` |
| `crates\vox-orchestrator-d` |
| `crates\vox-orchestrator-d\src\bin` |
| `crates\vox-orchestrator-mcp` |
| `crates\vox-orchestrator-mcp\src\services` |
| `crates\vox-orchestrator-mcp\tests` |
| `crates\vox-orchestrator-queue` |
| `crates\vox-orchestrator-test-helpers` |
| `crates\vox-orchestrator-test-helpers\src` |
| `crates\vox-orchestrator\.vox\memory\global` |
| `crates\vox-orchestrator\schema` |
| `crates\vox-orchestrator\src\planning\prompts` |
| `crates\vox-package-types` |
| `crates\vox-package\src\bin` |
| `crates\vox-plugin-api` |
| `crates\vox-plugin-grammar-export\src` |
| `crates\vox-plugin-host` |
| `crates\vox-plugin-host\tests\fixtures\noop-code-bad-abi\src` |
| `crates\vox-plugin-host\tests\fixtures\noop-code\src` |
| `crates\vox-plugin-populi-mesh\src\transport\store` |
| `crates\vox-plugin-types` |
| `crates\vox-plugin-webhook\src` |
| `crates\vox-prereg` |
| `crates\vox-primitives` |
| `crates\vox-project-scaffold` |
| `crates\vox-project-scaffold\src` |
| `crates\vox-protocol` |
| `crates\vox-protocol\src` |
| `crates\vox-publisher` |
| `crates\vox-publisher\tests\common` |
| `crates\vox-repository` |
| `crates\vox-reqwest-defaults` |
| `crates\vox-reqwest-defaults\src` |
| `crates\vox-research-events` |
| `crates\vox-ro-crate` |
| `crates\vox-rule-pack` |
| `crates\vox-rule-pack\examples` |
| `crates\vox-rule-pack\tests` |
| `crates\vox-scaling-policy` |
| `crates\vox-scientia-ingest` |
| `crates\vox-search` |
| `crates\vox-secrets` |
| `crates\vox-secrets\tests` |
| `crates\vox-skill-runtime` |
| `crates\vox-skills` |
| `crates\vox-skills\skills` |
| `crates\vox-ssg` |
| `crates\vox-ssg\src` |
| `crates\vox-telemetry` |
| `crates\vox-tensor` |
| `crates\vox-test-harness` |
| `crates\vox-wasm-engine` |
| `crates\vox-webhook` |
| `crates\vox-wire-format-validator` |
| `crates\vox-workflow-runtime` |
| `crates\vox-workflow-runtime\tests` |
| `crates\workspace-hack\src` |
| `docker` |
| `docs-astro\public\contributors` |
| `docs-astro\scripts` |
| `docs-astro\src\assets` |
| `docs-astro\src\components` |
| `docs-astro\src\pages` |
| `docs-astro\src\utils` |
| `docs\ci` |
| `docs\examples\.vox` |
| `docs\news` |
| `docs\src\architecture\prompts` |
| `docs\src\case-studies` |
| `docs\src\examples` |
| `docs\superpowers\plans\mental-tracker` |
| `docs\superpowers\specs` |
| `examples\golden\mesh` |
| `examples\sandboxes\test_app` |
| `examples\sandboxes\test_app\src` |
| `mens\data\heldout_bench` |
| `patches` |
| `patches\aegis-0.9.8\.cargo` |
| `patches\aegis-0.9.8\.github\workflows` |
| `patches\aegis-0.9.8\benches` |
| `patches\aegis-0.9.8\src\compat` |
| `patches\aegis-0.9.8\tests` |
| `patches\aegis-0.9.8\wasm-libs` |
| `scratch` |
| `scripts\populi` |
| `scripts\windows` |
| `tests\fixtures` |
