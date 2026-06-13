# Semantic Behavior Map — `vox-corpus`

Deterministically synthesized from 88 distinct proven-behavior claims (of 88 extracted) across 39 symbols. 1 symbols have an explicit error-path proof; **27 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `generate_all()`  (edge, happy, invariant; EXTRACTED)
- [invariant] produces output containing every entry from TOOL_REGISTRY_SLIM  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [invariant] produces output containing all A2A_MESSAGE_TYPES  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [invariant] produces output containing all SKILL_TOOLS  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [invariant] produces output containing all ORCHESTRATOR_TOOLS  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [happy] produces valid JSON lines with prompt, response, and category fields  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [happy] produces more than 500 training pairs by default  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [happy] excludes vox_build_crate when all trace generators are disabled  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [happy] excludes a2a_trace category when emit_a2a_traces is false  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [happy] produces at least min_phrasings_per_tool pairs for each tool  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [happy] produces at least one workflow response containing the word 'workflow'  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [happy] The synthetic generation output contains all entries from SKILL_TOOLS constant  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [happy] The synthetic generation output contains all orchestrator tool names from ORCHESTRATOR_TOOLS constant  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- … +7 more claims

### `analyse_str_with_taxonomy()`  (edge, happy; EXTRACTED)
- [happy] analyse_str_with_taxonomy counts each category occurrence and tracks total pairs in Report  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] analyse_str_with_taxonomy properly compares taxonomy against parsed category field  (crates/vox-corpus/src/corpus/coverage.rs)
- [edge] analyse_str_with_taxonomy normalizes 'rust_' prefixed category names to match taxonomy  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] Correctly counts covered types and identifies missing types from taxonomy against input JSONL  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [happy] Tallies total_pairs and per-category counts accurately from JSONL input  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] Identifies taxonomy types absent from input, excludes types present in input  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] Detects types below minimum pair threshold and lists them in underrepresented_types  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] Normalizes language-prefixed categories (rust_parser -> parser) and counts correctly  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] analyse_str_with_taxonomy() correctly counts 1 covered type when 1 of 2 taxonomy entries is present in JSONL  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [happy] analyse_str_with_taxonomy() correctly identifies missing taxonomy types  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [happy] CoverageReport correctly sums total_pairs to 4 from 4 JSONL input lines  (crates/vox-corpus/src/corpus/coverage.rs)

### `extract_from_source()`  (edge, happy; EXTRACTED)
- [happy] extract_from_source extracts documented Rust functions as training pairs  (crates/vox-corpus/src/corpus/extract_rs.rs)
- [happy] extract_from_source respects skip_tests config to exclude test functions  (crates/vox-corpus/src/corpus/extract_rs.rs)
- [happy] extract_from_source skips functions inside #[cfg(test)] modules when skip_tests=true  (crates/vox-corpus/src/corpus/extract_rs.rs)
- [edge] extract_from_source excludes functions with body shorter than min_body_lines threshold  (crates/vox-corpus/src/corpus/extract_rs.rs)
- [happy] Extracts documented functions with doc-comment text in prompt and function source in response  (crates/vox-corpus/src/corpus/extract_rs.rs)
- [happy] Excludes test functions from extraction when skip_tests config is true  (crates/vox-corpus/src/corpus/extract_rs.rs)

### `CoverageReport`  (happy; EXTRACTED)
- [happy] CoverageReport.counts maps each category to its occurrence count  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] CoverageReport.missing_types contains types with zero pairs in corpus  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] CoverageReport.missing_types does not contain types that have at least one pair  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] CoverageReport.underrepresented_types identifies types with non-zero but below-threshold pair counts  (crates/vox-corpus/src/corpus/coverage.rs)

### `extract_from_source function`  (edge, happy; EXTRACTED)
- [happy] extract_from_source extracts documented Rust functions when skip_tests=true  (crates/vox-corpus/src/corpus/extract_rs.rs)
- [happy] extract_from_source respects skip_tests=true to exclude test functions from output  (crates/vox-corpus/src/corpus/extract_rs.rs)
- [edge] extract_from_source filters out functions with fewer body lines than min_body_lines threshold (4 lines)  (crates/vox-corpus/src/corpus/extract_rs.rs)
- [edge] extract_from_source returns empty result when no functions meet min_body_lines requirement  (crates/vox-corpus/src/corpus/extract_rs.rs)

### `Block.metadata`  (happy; EXTRACTED)
- [happy] extracted Block.metadata[chunk_kind] is set to 'code_block'  (crates/vox-corpus/src/corpus/extract_docs.rs)
- [happy] extracted Q&A Block.metadata[chunk_kind] is set to 'qa_section'  (crates/vox-corpus/src/corpus/extract_docs.rs)

### `Report struct / missing_types field`  (edge, invariant; EXTRACTED)
- [edge] Report.missing_types contains taxonomy entries not present in input data ('actor', 'workflow' missing from single 'function' entry)  (crates/vox-corpus/src/corpus/coverage.rs)
- [invariant] Report.missing_types is empty when all taxonomy entries are covered  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report.balance_score`  (happy; EXTRACTED)
- [happy] Report.balance_score equals 1.0 when all categories have equal representation  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] Achieves balance_score of 1.0 for uniformly distributed category counts  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report.coverage_ratio`  (happy; EXTRACTED)
- [happy] Report.coverage_ratio equals 1.0 when all taxonomy categories are present  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] Equals 1.0 when all taxonomy types are present in input, missing_types is empty  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report.is_sufficient()`  (happy; EXTRACTED)
- [happy] Report.is_sufficient() returns true when all taxonomy categories meet min count threshold  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] Returns true when all taxonomy types meet minimum pair threshold  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report.missing_types`  (happy; EXTRACTED)
- [happy] Report.missing_types identifies categories in taxonomy not present in JSONL input  (crates/vox-corpus/src/corpus/coverage.rs)
- [happy] Report.missing_types is empty when all taxonomy categories appear in JSONL  (crates/vox-corpus/src/corpus/coverage.rs)

### `TOOL_REGISTRY_SLIM`  (invariant; EXTRACTED)
- [invariant] contains all entries from ORCHESTRATOR_TOOLS as a subset  (crates/vox-corpus/tests/synthetic_gen_test.rs)
- [invariant] All entries in ORCHESTRATOR_TOOLS constant are present in TOOL_REGISTRY_SLIM constant  (crates/vox-corpus/tests/synthetic_gen_test.rs)

### `extract_code_blocks()`  (happy; EXTRACTED)
- [happy] extract_code_blocks extracts vox code blocks from markdown and populates output vector  (crates/vox-corpus/src/corpus/extract_docs.rs)
- [happy] Extracts vox code blocks with correct response content and chunk_kind metadata  (crates/vox-corpus/src/corpus/extract_docs.rs)

### `extract_qa_sections()`  (happy; EXTRACTED)
- [happy] extract_qa_sections extracts Q&A pairs from markdown sections above min_section_chars threshold  (crates/vox-corpus/src/corpus/extract_docs.rs)
- [happy] Extracts Q&A sections with prompt text and chunk_kind metadata label  (crates/vox-corpus/src/corpus/extract_docs.rs)

### `validate_external_review_rows()`  (error, happy; EXTRACTED)
- [error] rejects rows with empty prompt field  (crates/vox-corpus/src/external_review_replay.rs)
- [happy] accepts rows with valid prompt, response, and required fields  (crates/vox-corpus/src/external_review_replay.rs)

### `Block struct / response field`  (happy; EXTRACTED)
- [happy] Extracted block response contains expected code fragment ('actor Counter')  (crates/vox-corpus/src/corpus/extract_docs.rs)

### `Block.prompt`  (happy; EXTRACTED)
- [happy] extracted Q&A Block.prompt contains section content matching original markdown  (crates/vox-corpus/src/corpus/extract_docs.rs)

### `Block.response`  (happy; EXTRACTED)
- [happy] extracted Block.response contains the vox code block content  (crates/vox-corpus/src/corpus/extract_docs.rs)

### `Report struct / balance_score field`  (invariant; EXTRACTED)
- [invariant] Report.balance_score equals 1.0 for uniform distribution across taxonomy categories  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report struct / counts field`  (happy; EXTRACTED)
- [happy] Report.counts correctly aggregates pair counts by category (2 'function', 1 'actor' from 4 pairs)  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report struct / coverage_ratio field`  (invariant; EXTRACTED)
- [invariant] Report.coverage_ratio equals 1.0 when all taxonomy types are represented  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report struct / is_sufficient() method`  (invariant; EXTRACTED)
- [invariant] Report.is_sufficient() returns true only when all taxonomy types meet minimum threshold and are present  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report struct / total_pairs field`  (happy; EXTRACTED)
- [happy] Report.total_pairs is set to the total number of processed pairs (4)  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report struct / underrepresented_types field`  (edge; EXTRACTED)
- [edge] Report.underrepresented_types identifies types below minimum threshold (1 'function' pair below threshold of 5)  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report.counts`  (happy; EXTRACTED)
- [happy] Report.counts correctly tallies occurrences per category from JSONL input  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report.total_pairs`  (happy; EXTRACTED)
- [happy] Report.total_pairs equals the number of input JSONL lines  (crates/vox-corpus/src/corpus/coverage.rs)

### `Report.underrepresented_types`  (edge; EXTRACTED)
- [edge] Report.underrepresented_types identifies categories below threshold count  (crates/vox-corpus/src/corpus/coverage.rs)

### `SKILL_TOOLS`  (happy; EXTRACTED)
- [happy] All entries in SKILL_TOOLS appear in synthetic generation output  (crates/vox-corpus/src/synthetic_gen/tests.rs)

### `TrainingPair.prompt`  (happy; EXTRACTED)
- [happy] TrainingPair.prompt is populated from doc comments of the function  (crates/vox-corpus/src/corpus/extract_rs.rs)

### `TrainingPair.response`  (happy; EXTRACTED)
- [happy] TrainingPair.response contains the extracted function source code  (crates/vox-corpus/src/corpus/extract_rs.rs)

### `analyse_str_with_taxonomy function`  (happy; EXTRACTED)
- [happy] analyse_str_with_taxonomy normalizes 'rust_' prefix in category names for taxonomy matching  (crates/vox-corpus/src/corpus/coverage.rs)

### `extract_code_blocks function`  (happy; EXTRACTED)
- [happy] extract_code_blocks successfully extracts vox code block from markdown and populates output vector  (crates/vox-corpus/src/corpus/extract_docs.rs)

### `extract_code_blocks function / metadata`  (happy; EXTRACTED)
- [happy] extract_code_blocks sets chunk_kind metadata to 'code_block'  (crates/vox-corpus/src/corpus/extract_docs.rs)

### `extract_from_source function / prompt field`  (happy; EXTRACTED)
- [happy] extract_from_source generates prompts from doc comments (contains 'sum')  (crates/vox-corpus/src/corpus/extract_rs.rs)

### `extract_from_source function / response field`  (happy; EXTRACTED)
- [happy] extract_from_source includes function definition source in response field  (crates/vox-corpus/src/corpus/extract_rs.rs)

### `extract_qa_sections function`  (happy; EXTRACTED)
- [happy] extract_qa_sections extracts Q&A sections from markdown above minimum character threshold  (crates/vox-corpus/src/corpus/extract_docs.rs)

### `extract_qa_sections function / metadata`  (happy; EXTRACTED)
- [happy] extract_qa_sections sets chunk_kind metadata to 'qa_section'  (crates/vox-corpus/src/corpus/extract_docs.rs)

### `extract_qa_sections function / prompt field`  (happy; EXTRACTED)
- [happy] extract_qa_sections generates prompts containing expected content ('Actor Model')  (crates/vox-corpus/src/corpus/extract_docs.rs)

### `generate_tool_pairs()`  (happy; EXTRACTED)
- [happy] generate_tool_pairs() produces at least min_phrasings_per_tool output lines for a given tool  (crates/vox-corpus/src/synthetic_gen/tests.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`Block struct / response field`** — only: _Extracted block response contains expected code fragment ('actor Counter')_
- **`Block.metadata`** — only: _extracted Block.metadata[chunk_kind] is set to 'code_block'_
- **`Block.prompt`** — only: _extracted Q&A Block.prompt contains section content matching original markdown_
- **`Block.response`** — only: _extracted Block.response contains the vox code block content_
- **`CoverageReport`** — only: _CoverageReport.counts maps each category to its occurrence count_
- **`Report struct / counts field`** — only: _Report.counts correctly aggregates pair counts by category (2 'function', 1 'actor' from 4 pairs)_
- **`Report struct / total_pairs field`** — only: _Report.total_pairs is set to the total number of processed pairs (4)_
- **`Report.balance_score`** — only: _Report.balance_score equals 1.0 when all categories have equal representation_
- **`Report.counts`** — only: _Report.counts correctly tallies occurrences per category from JSONL input_
- **`Report.coverage_ratio`** — only: _Report.coverage_ratio equals 1.0 when all taxonomy categories are present_
- **`Report.is_sufficient()`** — only: _Report.is_sufficient() returns true when all taxonomy categories meet min count threshold_
- **`Report.missing_types`** — only: _Report.missing_types identifies categories in taxonomy not present in JSONL input_
- **`Report.total_pairs`** — only: _Report.total_pairs equals the number of input JSONL lines_
- **`SKILL_TOOLS`** — only: _All entries in SKILL_TOOLS appear in synthetic generation output_
- **`TrainingPair.prompt`** — only: _TrainingPair.prompt is populated from doc comments of the function_
- **`TrainingPair.response`** — only: _TrainingPair.response contains the extracted function source code_
- **`analyse_str_with_taxonomy function`** — only: _analyse_str_with_taxonomy normalizes 'rust_' prefix in category names for taxonomy matching_
- **`extract_code_blocks function`** — only: _extract_code_blocks successfully extracts vox code block from markdown and populates output vector_
- **`extract_code_blocks function / metadata`** — only: _extract_code_blocks sets chunk_kind metadata to 'code_block'_
- **`extract_code_blocks()`** — only: _extract_code_blocks extracts vox code blocks from markdown and populates output vector_
- **`extract_from_source function / prompt field`** — only: _extract_from_source generates prompts from doc comments (contains 'sum')_
- **`extract_from_source function / response field`** — only: _extract_from_source includes function definition source in response field_
- **`extract_qa_sections function`** — only: _extract_qa_sections extracts Q&A sections from markdown above minimum character threshold_
- **`extract_qa_sections function / metadata`** — only: _extract_qa_sections sets chunk_kind metadata to 'qa_section'_
- **`extract_qa_sections function / prompt field`** — only: _extract_qa_sections generates prompts containing expected content ('Actor Model')_
- **`extract_qa_sections()`** — only: _extract_qa_sections extracts Q&A pairs from markdown sections above min_section_chars threshold_
- **`generate_tool_pairs()`** — only: _generate_tool_pairs() produces at least min_phrasings_per_tool output lines for a given tool_
