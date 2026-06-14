# Reached-but-NOT-Proven — Phase 0 (llvm-cov × proven map)

Annotated 26323 code symbols with llvm-cov `reached` status.

**reached-not-proven** = a symbol whose code EXECUTED during tests but has NO asserted behavior (`proves` edge). This is the precise set line coverage counts as 'covered' but that proves nothing — the keystone signal of this whole initiative.


**Total reached-but-unproven symbols: 3**


| Crate | Code | Reached | Proven | Reached-not-proven |
|---|---|---|---|---|
| vox-plugin-sdk | 12 | 5 | 2 | **3** |
| vox-actor-runtime | 663 | 0 | 103 | **0** |
| vox-arch-check | 89 | 0 | 20 | **0** |
| vox-ast | 243 | 0 | 24 | **0** |
| vox-audit | 392 | 0 | 80 | **0** |
| vox-bounded-fs | 6 | 0 | 2 | **0** |
| vox-build-meta | 3 | 0 | 1 | **0** |
| vox-capability-registry | 51 | 0 | 3 | **0** |
| vox-cli | 3463 | 0 | 650 | **0** |
| vox-cli-ci | 81 | 0 | 35 | **0** |
| vox-cli-core | 150 | 0 | 5 | **0** |
| vox-cli-tests | 19 | 0 | 1 | **0** |
| vox-code-audit | 1530 | 0 | 343 | **0** |
| vox-codegen | 672 | 0 | 80 | **0** |
| vox-codegen-ts | 403 | 0 | 44 | **0** |
| vox-compiler | 1760 | 0 | 392 | **0** |
| vox-config | 273 | 0 | 63 | **0** |
| vox-constrained-gen | 82 | 0 | 13 | **0** |
| vox-container | 34 | 0 | 0 | **0** |
| vox-container-types | 48 | 0 | 18 | **0** |
| vox-corpus | 431 | 0 | 29 | **0** |
| vox-crypto | 39 | 0 | 13 | **0** |
| vox-db | 1694 | 0 | 237 | **0** |
| vox-db-types | 165 | 0 | 4 | **0** |
| vox-deploy-codegen | 30 | 0 | 1 | **0** |
| vox-doc-inventory | 49 | 0 | 2 | **0** |
| vox-doc-pipeline | 41 | 0 | 5 | **0** |
| vox-drift-check | 279 | 0 | 47 | **0** |
| vox-effort-audit | 149 | 0 | 48 | **0** |
| vox-effort-route | 153 | 0 | 37 | **0** |
| vox-eval | 23 | 0 | 3 | **0** |
| vox-forge | 134 | 0 | 1 | **0** |
| vox-foundation | 25 | 0 | 3 | **0** |
| vox-gamify | 744 | 0 | 69 | **0** |
| vox-git | 65 | 0 | 11 | **0** |
| vox-grammar-export | 49 | 0 | 10 | **0** |
| vox-gui | 474 | 0 | 43 | **0** |
| vox-hf-layout | 26 | 0 | 6 | **0** |
| vox-http-client | 25 | 0 | 6 | **0** |
| vox-identity | 61 | 0 | 3 | **0** |
| vox-integration-tests | 1 | 0 | 0 | **0** |
| vox-journal | 25 | 0 | 6 | **0** |
| vox-jsonschema-util | 25 | 0 | 3 | **0** |
| vox-langtool | 18 | 0 | 6 | **0** |
| vox-lsp | 99 | 0 | 7 | **0** |
| vox-mcp-registry | 9 | 0 | 0 | **0** |
| vox-mesh-policy | 33 | 0 | 1 | **0** |
| vox-mesh-types | 120 | 0 | 20 | **0** |
| vox-ml-cli | 426 | 0 | 32 | **0** |
| vox-openai | 31 | 0 | 3 | **0** |
| vox-openclaw-runtime | 147 | 0 | 5 | **0** |
| vox-orchestrator | 3564 | 0 | 641 | **0** |
| vox-orchestrator-d | 4 | 0 | 0 | **0** |
| vox-orchestrator-driver | 13 | 0 | 2 | **0** |
| vox-orchestrator-mcp | 1912 | 0 | 182 | **0** |
| vox-orchestrator-queue | 305 | 0 | 47 | **0** |
| vox-orchestrator-test-helpers | 18 | 0 | 0 | **0** |
| vox-orchestrator-types | 142 | 0 | 14 | **0** |
| vox-package | 84 | 0 | 6 | **0** |
| vox-package-types | 129 | 0 | 11 | **0** |

## Top reached-but-unproven symbols (per worst crate)


### vox-plugin-sdk
- `P` — crates/vox-plugin-sdk/src/lib.rs:L49
- `VoxPluginRef` — crates/vox-plugin-sdk/src/lib.rs:L49
- `RString` — crates/vox-plugin-sdk/src/lib.rs:L62
