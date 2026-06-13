# Semantic Behavior Map — `vox-effort-route`

Deterministically synthesized from 89 distinct proven-behavior claims (of 89 extracted) across 38 symbols. 1 symbols have an explicit error-path proof; **16 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `run`  (edge, happy; EXTRACTED)
- [happy] run() returns summary with accurate findings_loaded count after filtering  (crates/vox-effort-route/src/pipeline.rs)
- [happy] run() creates recommendations.jsonl output file with one line per routed cluster  (crates/vox-effort-route/src/pipeline.rs)
- [happy] run() creates recommendations.md output file  (crates/vox-effort-route/src/pipeline.rs)
- [happy] run() with high-confidence mock router verifies all routed clusters  (crates/vox-effort-route/src/pipeline.rs)
- [edge] run() with zero token budget skips all clusters after first exceeds budget  (crates/vox-effort-route/src/pipeline.rs)
- [happy] run() writes all budget-skipped clusters to recommendations.jsonl output  (crates/vox-effort-route/src/pipeline.rs)
- [happy] run() does not draft artifacts for budget-skipped clusters  (crates/vox-effort-route/src/pipeline.rs)
- [edge] run() respects dollar budget ceiling and skips subsequent clusters  (crates/vox-effort-route/src/pipeline.rs)
- [happy] run() tracks actual cost from router as total_judge_cost_usd  (crates/vox-effort-route/src/pipeline.rs)
- [happy] run() returns None for total_judge_cost_usd when using mock router  (crates/vox-effort-route/src/pipeline.rs)

### `render()`  (happy, invariant; EXTRACTED)
- [invariant] render() does not emit author identity symbols (@ character) in markdown output  (crates/vox-effort-route/src/emit/markdown.rs)
- [invariant] render() does not emit author identity as hex-encoded sequences in markdown output  (crates/vox-effort-route/src/emit/markdown.rs)
- [happy] render() ranks clusters with higher token counts first among verified clusters  (crates/vox-effort-route/src/emit/markdown.rs)
- [happy] render() places verified clusters before unverified clusters in output  (crates/vox-effort-route/src/emit/markdown.rs)
- [happy] render() emits 'unknown (model not in pricing catalog)' message when judge costs are None  (crates/vox-effort-route/src/emit/markdown.rs)
- [invariant] render() does not emit '$0.00' when judge costs are not available  (crates/vox-effort-route/src/emit/markdown.rs)
- [happy] render() sums known judge costs and emits the total (e.g., '$0.15' for 0.12 + 0.03)  (crates/vox-effort-route/src/emit/markdown.rs)

### `write_artifact()`  (edge, happy; EXTRACTED)
- [happy] write_artifact writes a decision to a file under the staging directory with .proposed extension  (crates/vox-effort-route/src/emit/artifacts.rs)
- [happy] write_artifact returns a path containing 'artifacts' directory component  (crates/vox-effort-route/src/emit/artifacts.rs)
- [happy] write_artifact writes the decision body to disk as file content  (crates/vox-effort-route/src/emit/artifacts.rs)
- [edge] write_artifact returns None when decision is unverified (verified=false)  (crates/vox-effort-route/src/emit/artifacts.rs)
- [edge] write_artifact returns None when artifact form is None  (crates/vox-effort-route/src/emit/artifacts.rs)

### `LlmEmbedder::config`  (happy; EXTRACTED)
- [happy] config() returns provider as 'auto' regardless of embedder model  (crates/vox-effort-route/src/embed.rs)
- [happy] config() returns the exact model passed to the embedder constructor  (crates/vox-effort-route/src/embed.rs)
- [happy] config() returns timeout_ms converted from Duration seconds  (crates/vox-effort-route/src/embed.rs)
- [happy] config() returns no response_format for embedding requests  (crates/vox-effort-route/src/embed.rs)

### `RecommendationRow`  (happy, invariant; EXTRACTED)
- [happy] RecommendationRow preserves cluster_id field during JSONL round-trip serialization  (crates/vox-effort-route/src/emit/jsonl.rs)
- [happy] RecommendationRow preserves total_member_tokens field during JSONL round-trip serialization  (crates/vox-effort-route/src/emit/jsonl.rs)
- [happy] RecommendationRow preserves artifact_form field during JSONL round-trip serialization  (crates/vox-effort-route/src/emit/jsonl.rs)
- [invariant] RecommendationRow schema_version is '1.0'  (crates/vox-effort-route/src/emit/mod.rs)

### `primary_crate()`  (edge, happy; EXTRACTED)
- [happy] primary_crate() returns the crate name for a bucket key when findings share the same remediation kind  (crates/vox-effort-route/src/bucket.rs)
- [happy] primary_crate() selects the crate with the most evidence pointers rather than the first one  (crates/vox-effort-route/src/bucket.rs)
- [edge] primary_crate() tie-breaks equal plurality counts by selecting the lexicographically smallest crate name  (crates/vox-effort-route/src/bucket.rs)
- [edge] primary_crate() returns '<workspace-root>' when all evidence pointers are non-crate files  (crates/vox-effort-route/src/bucket.rs)

### `vox_effort_route::run`  (happy, invariant; EXTRACTED)
- [happy] run() creates recommendations.jsonl output file in staging directory  (crates/vox-effort-route/tests/e2e.rs)
- [happy] run() creates recommendations.md output file in staging directory  (crates/vox-effort-route/tests/e2e.rs)
- [happy] run() routes at least one cluster and verifies at least one recommendation  (crates/vox-effort-route/tests/e2e.rs)
- [invariant] run() does not leak author email addresses to recommendations.md  (crates/vox-effort-route/tests/e2e.rs)

### `write_artifact`  (happy; EXTRACTED)
- [happy] write_artifact() creates file under staging directory  (crates/vox-effort-route/src/emit/artifacts.rs)
- [happy] write_artifact() creates file with .proposed extension  (crates/vox-effort-route/src/emit/artifacts.rs)
- [happy] write_artifact() creates file under artifacts subdirectory  (crates/vox-effort-route/src/emit/artifacts.rs)
- [happy] write_artifact() writes decision body to artifact file  (crates/vox-effort-route/src/emit/artifacts.rs)

### `Count field (summary.clusters_routed)`  (happy, invariant; EXTRACTED)
- [invariant] clusters_routed count matches the number of lines in recommendations.jsonl output  (crates/vox-effort-route/src/pipeline.rs)
- [happy] clusters_routed is at least 1 when using a dollar budget that allows first cluster to route  (crates/vox-effort-route/src/pipeline.rs)
- [happy] clusters_routed is at least 1 with 2 surviving findings  (crates/vox-effort-route/tests/e2e.rs)

### `read`  (error, happy; EXTRACTED)
- [happy] read() filters out rows with null findings  (crates/vox-effort-route/src/load.rs)
- [happy] read() filters out rows with waste_score below threshold  (crates/vox-effort-route/src/load.rs)
- [error] read() returns LoadError::SchemaMismatch when schema_version does not match  (crates/vox-effort-route/src/load.rs)

### `run() function`  (happy, invariant; EXTRACTED)
- [happy] run() loads fixture findings, filters out null and low-score entries, and returns a summary showing 2 surviving findings loaded, at least 1 cluster routed, and generates recommendations.jsonl and recommendations.md output files  (crates/vox-effort-route/src/pipeline.rs)
- [invariant] run() still emits complete, honest recommendations.jsonl with all clusters even when budget is exhausted (not truncated)  (crates/vox-effort-route/src/pipeline.rs)
- [happy] run() with fixture findings produces recommendations.jsonl and recommendations.md output files  (crates/vox-effort-route/tests/e2e.rs)

### `Cluster ordering in render() output`  (happy, invariant; EXTRACTED)
- [invariant] render() places verified clusters before unverified clusters in markdown output  (crates/vox-effort-route/src/emit/markdown.rs)
- [happy] render() sorts verified clusters by token count in descending order (higher tokens first)  (crates/vox-effort-route/src/emit/markdown.rs)

### `Count field (summary.clusters_skipped_over_budget)`  (edge, happy; EXTRACTED)
- [happy] clusters_skipped_over_budget is 0 when using default config with MockRouter at 0.9 confidence  (crates/vox-effort-route/src/pipeline.rs)
- [edge] clusters_skipped_over_budget equals clusters_routed when max_total_tokens budget is 0, meaning all clusters are budget-skipped  (crates/vox-effort-route/src/pipeline.rs)

### `Count field (summary.verified)`  (edge, happy; EXTRACTED)
- [edge] verified count is 0 when token budget is exhausted  (crates/vox-effort-route/src/pipeline.rs)
- [happy] verified count is at least 1 with fixture findings and MockRouter at 0.9 confidence  (crates/vox-effort-route/tests/e2e.rs)

### `JsonlWriter`  (happy, invariant; EXTRACTED)
- [invariant] JsonlWriter includes schema_version field '1.0' in each emitted line  (crates/vox-effort-route/src/emit/jsonl.rs)
- [happy] JsonlWriter output can be parsed back as RecommendationRow structs with correct schema_version  (crates/vox-effort-route/src/emit/jsonl.rs)

### `JsonlWriter::append()`  (happy; EXTRACTED)
- [happy] JsonlWriter.append() writes one line per row call to the JSONL file  (crates/vox-effort-route/src/emit/jsonl.rs)
- [happy] JsonlWriter.append writes each row as exactly one line in JSONL format  (crates/vox-effort-route/src/emit/jsonl.rs)

### `RecommendationRow::new()`  (happy; EXTRACTED)
- [happy] RecommendationRow::new() stamps the schema_version field with SCHEMA_VERSION constant  (crates/vox-effort-route/src/emit/mod.rs)
- [happy] RecommendationRow::new() preserves the cluster_id from decision input  (crates/vox-effort-route/src/emit/mod.rs)

### `cosine_distance`  (happy; EXTRACTED)
- [happy] cosine_distance of identical vectors returns near-zero  (crates/vox-effort-route/src/cluster.rs)
- [happy] cosine_distance of orthogonal vectors returns near-one  (crates/vox-effort-route/src/cluster.rs)

### `maybe_split`  (happy; EXTRACTED)
- [happy] Buckets with more members than threshold are split into multiple sub-clusters by embedding vectors  (crates/vox-effort-route/src/cluster.rs)
- [happy] Each sub-cluster after split contains equal members across distinct embedding axes  (crates/vox-effort-route/src/cluster.rs)

### `maybe_split()`  (happy; EXTRACTED)
- [happy] maybe_split() does not call embedder when bucket size is below threshold  (crates/vox-effort-route/src/cluster.rs)
- [happy] maybe_split() splits buckets into sub-clusters based on distinct embedding vectors when size exceeds threshold  (crates/vox-effort-route/src/cluster.rs)

### `render() function`  (invariant; EXTRACTED)
- [invariant] render() does not emit @ symbol in markdown output (no email addresses)  (crates/vox-effort-route/src/emit/markdown.rs)
- [invariant] render() does not emit 64-byte sequences of all hex digits (no hashes) in markdown output  (crates/vox-effort-route/src/emit/markdown.rs)

### `write_artifact() function`  (edge, happy; EXTRACTED)
- [happy] write_artifact() for verified CiGate decision returns Some(path) to written file  (crates/vox-effort-route/src/emit/artifacts.rs)
- [edge] write_artifact() for unverified CiGate decision returns None  (crates/vox-effort-route/src/emit/artifacts.rs)

### `ArtifactForm deserialization`  (happy; EXTRACTED)
- [happy] ArtifactForm enum value (CiGate) deserializes correctly from JSON JSONL row  (crates/vox-effort-route/src/emit/jsonl.rs)

### `ArtifactForm file paths`  (invariant; EXTRACTED)
- [invariant] write_artifact() writes files under the artifacts subdirectory with .proposed extension  (crates/vox-effort-route/src/emit/artifacts.rs)

### `ArtifactForm::CiGate`  (happy; EXTRACTED)
- [happy] write_artifact() writes the decision body content correctly to the output file  (crates/vox-effort-route/src/emit/artifacts.rs)

### `ArtifactForm::None`  (edge; EXTRACTED)
- [edge] write_artifact() for None form decision returns None even when verified  (crates/vox-effort-route/src/emit/artifacts.rs)

### `Bucket`  (happy; EXTRACTED)
- [happy] Bucket groups findings with identical (waste_category, remediation_kind, crate) keys into a single bucket  (crates/vox-effort-route/src/bucket.rs)

### `Count field (lines)`  (invariant; EXTRACTED)
- [invariant] Each line in JSONL output contains schema_version field with value 1.0  (crates/vox-effort-route/src/emit/jsonl.rs)

### `Count field (summary.judge_tokens_spent)`  (edge; EXTRACTED)
- [edge] judge_tokens_spent is 0 when max_total_tokens budget is 0  (crates/vox-effort-route/src/pipeline.rs)

### `Count field (summary.total_judge_cost_usd)`  (invariant; EXTRACTED)
- [invariant] total_judge_cost_usd is None (not 0.00) when using MockRouter which performs no real LLM I/O  (crates/vox-effort-route/src/pipeline.rs)

### `JsonlWriter and RecommendationRow round-trip`  (happy; EXTRACTED)
- [happy] RecommendationRow can be serialized and deserialized from JSONL format correctly  (crates/vox-effort-route/src/emit/jsonl.rs)

### `LlmEmbedder.config()`  (invariant; EXTRACTED)
- [invariant] LlmEmbedder.config() returns provider='auto', uses caller-resolved model id, includes timeout_ms, and never sets response_format  (crates/vox-effort-route/src/embed.rs)

### `ModelRates::cost_usd`  (invariant; EXTRACTED)
- [invariant] cost_usd() returns None for unknown model rates instead of zero  (crates/vox-effort-route/src/pricing.rs)

### `ModelRates::cost_usd()`  (happy; EXTRACTED)
- [happy] ModelRates::default().cost_usd() returns None for unknown model rates, not 0.00  (crates/vox-effort-route/src/pricing.rs)

### `actor`  (happy; INFERRED)
- [happy] Evidence pointers can come from different crates and the plurality-picking logic considers their distribution  (crates/vox-effort-route/src/bucket.rs)

### `cosine_distance()`  (happy; EXTRACTED)
- [happy] cosine_distance() returns near-zero for identical vectors and near-one for orthogonal vectors  (crates/vox-effort-route/src/cluster.rs)

### `crate_from_path()`  (happy; EXTRACTED)
- [happy] crate_from_path() returns Some(crate_name) for evidence pointers in crate paths like 'crates/vox-config/src/timeouts.rs:8' and returns None for non-crate files like 'README.md'  (crates/vox-effort-route/src/bucket.rs)

### `row()`  (happy; EXTRACTED)
- [happy] LoadedFinding.row can be modified to contain multiple evidence pointers  (crates/vox-effort-route/src/bucket.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`ArtifactForm deserialization`** — only: _ArtifactForm enum value (CiGate) deserializes correctly from JSON JSONL row_
- **`ArtifactForm::CiGate`** — only: _write_artifact() writes the decision body content correctly to the output file_
- **`Bucket`** — only: _Bucket groups findings with identical (waste_category, remediation_kind, crate) keys into a single bucket_
- **`JsonlWriter and RecommendationRow round-trip`** — only: _RecommendationRow can be serialized and deserialized from JSONL format correctly_
- **`JsonlWriter::append()`** — only: _JsonlWriter.append() writes one line per row call to the JSONL file_
- **`LlmEmbedder::config`** — only: _config() returns provider as 'auto' regardless of embedder model_
- **`ModelRates::cost_usd()`** — only: _ModelRates::default().cost_usd() returns None for unknown model rates, not 0.00_
- **`RecommendationRow::new()`** — only: _RecommendationRow::new() stamps the schema_version field with SCHEMA_VERSION constant_
- **`actor`** — only: _Evidence pointers can come from different crates and the plurality-picking logic considers their distribution_
- **`cosine_distance`** — only: _cosine_distance of identical vectors returns near-zero_
- **`cosine_distance()`** — only: _cosine_distance() returns near-zero for identical vectors and near-one for orthogonal vectors_
- **`crate_from_path()`** — only: _crate_from_path() returns Some(crate_name) for evidence pointers in crate paths like 'crates/vox-config/src/timeouts.rs:8' and returns None for non-crate files like 'README.md'_
- **`maybe_split`** — only: _Buckets with more members than threshold are split into multiple sub-clusters by embedding vectors_
- **`maybe_split()`** — only: _maybe_split() does not call embedder when bucket size is below threshold_
- **`row()`** — only: _LoadedFinding.row can be modified to contain multiple evidence pointers_
- **`write_artifact`** — only: _write_artifact() creates file under staging directory_
