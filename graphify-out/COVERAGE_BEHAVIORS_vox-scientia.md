# Semantic Behavior Map — `vox-scientia`

Deterministically synthesized from 243 distinct proven-behavior claims (of 243 extracted) across 84 symbols. 14 symbols have an explicit error-path proof; **38 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `next_state`  (edge, happy, invariant; EXTRACTED)
- [happy] next_state transitions ReviewState from Surfaced to UnderReview when given ReviewAction::StartReview  (crates/vox-scientia/src/review/mod.rs)
- [happy] next_state transitions ReviewState from UnderReview to Approved when given ReviewAction::Approve  (crates/vox-scientia/src/review/mod.rs)
- [happy] next_state transitions ReviewState from UnderReview to Rejected when given ReviewAction::Reject  (crates/vox-scientia/src/review/mod.rs)
- [happy] next_state transitions ReviewState from UnderReview to Deferred when given ReviewAction::Defer  (crates/vox-scientia/src/review/mod.rs)
- [happy] next_state() with state=UnderReview and action=Reject returns ReviewState::Rejected  (crates/vox-scientia/src/review/mod.rs)
- [happy] next_state() with state=UnderReview and action=Defer returns ReviewState::Deferred  (crates/vox-scientia/src/review/mod.rs)
- [happy] next_state() with state=UnderReview and action=Edit returns ReviewState::Edited  (crates/vox-scientia/src/review/mod.rs)
- [happy] next_state() with state=Edited and action=ReSurface returns ReviewState::Surfaced  (crates/vox-scientia/src/review/mod.rs)
- [happy] next_state() with state=Deferred and action=ReSurface returns ReviewState::Surfaced  (crates/vox-scientia/src/review/mod.rs)
- [edge] next_state() with state=Surfaced and action=Approve returns ReviewState::Surfaced (no-op, cannot approve directly)  (crates/vox-scientia/src/review/mod.rs)
- [edge] next_state() with state=Surfaced and action=Reject returns ReviewState::Surfaced (no-op)  (crates/vox-scientia/src/review/mod.rs)
- [edge] next_state() with state=Surfaced and action=Defer returns ReviewState::Surfaced (no-op)  (crates/vox-scientia/src/review/mod.rs)
- … +6 more claims

### `mint_from_decision`  (edge, happy; EXTRACTED)
- [happy] mint_from_decision returns Some variant for approved decision, and the token carries claim_id, publication_id, and bound_digest values from the input decision  (crates/vox-scientia/src/review/mod.rs)
- [happy] mint_from_decision copies the publication_id field from input decision into the returned token  (crates/vox-scientia/src/review/mod.rs)
- [happy] mint_from_decision returns None variant when decision field is rejected  (crates/vox-scientia/src/review/mod.rs)
- [happy] mint_from_decision returns None variant when decision field is deferred  (crates/vox-scientia/src/review/mod.rs)
- [happy] mint_from_decision returns None variant when decision field is edited  (crates/vox-scientia/src/review/mod.rs)
- [happy] mint_from_decision returns None variant when decision field is an unknown/unrecognized value  (crates/vox-scientia/src/review/mod.rs)
- [happy] token returned by mint_from_decision exposes claim_id and bound_digest accessors that return the values from the input decision  (crates/vox-scientia/src/review/mod.rs)
- [happy] mint_from_decision works with ReviewDecisionRow implementing ReviewDecisionLike trait, returning a token with claim_id and bound_digest from the row  (crates/vox-scientia/src/review/mod.rs)
- [happy] Returns Some token with decision's claim_id, publication_id, and bound_digest when decision is approved  (crates/vox-scientia/src/review/mod.rs)
- [happy] Carries publication_id from decision into minted token's publication_id accessor  (crates/vox-scientia/src/review/mod.rs)
- [happy] Returns None when decision is rejected  (crates/vox-scientia/src/review/mod.rs)
- [happy] Returns None when decision is deferred  (crates/vox-scientia/src/review/mod.rs)
- … +2 more claims

### `AtomicNoveltyScorer::score`  (edge, happy; EXTRACTED)
- [edge] Returns NoveltyVerdict::Novel for an empty bundle  (crates/vox-scientia/src/inspect_bridge/novelty.rs)
- [happy] Returns NoveltyVerdict::Novel when similarity score is low (0.3)  (crates/vox-scientia/src/inspect_bridge/novelty.rs)
- [happy] Returns NoveltyVerdict::NotNovel with correct URI and similarity when score is high (0.85)  (crates/vox-scientia/src/inspect_bridge/novelty.rs)
- [happy] Returns NoveltyVerdict::PossiblyNovel with correct closest_score for mid-range similarity (0.65)  (crates/vox-scientia/src/inspect_bridge/novelty.rs)
- [edge] Returns NoveltyVerdict::Novel when bundle has no semantic score or summary  (crates/vox-scientia/src/inspect_bridge/novelty.rs)
- [happy] when scoring an empty bundle with no hits, returns NoveltyVerdict::Novel  (crates/vox-scientia/src/inspect_bridge/novelty.rs)
- [happy] when scoring a bundle with low similarity score (0.3), returns NoveltyVerdict::Novel  (crates/vox-scientia/src/inspect_bridge/novelty.rs)
- [happy] when scoring a bundle with high similarity score (0.85), returns NoveltyVerdict::NotNovel with correct closest_hit_uri and similarity fields  (crates/vox-scientia/src/inspect_bridge/novelty.rs)
- [happy] when scoring a bundle with mid-range similarity score (0.65), returns NoveltyVerdict::PossiblyNovel with correct closest_score  (crates/vox-scientia/src/inspect_bridge/novelty.rs)
- [happy] when scoring a bundle with no summary and no scores, returns NoveltyVerdict::Novel  (crates/vox-scientia/src/inspect_bridge/novelty.rs)

### `render_imrad`  (edge, happy, invariant; EXTRACTED)
- [happy] render_imrad output includes numbered figure headings, markdown image embeds, TODO caption blocks, and provenance footers with SHA3 hash and source script  (crates/vox-scientia/src/manuscript/scaffold/render.rs)
- [happy] render_imrad numbers and orders multiple figure entries in the same sequence as the input array  (crates/vox-scientia/src/manuscript/scaffold/render.rs)
- [happy] When a figure has no caption_hint, the rendered markdown omits the 'machine-suggested; not authoritative' text  (crates/vox-scientia/src/manuscript/scaffold/render.rs)
- [happy] When a figure has no caption_hint, the rendered markdown still includes the TODO(figure-caption) block  (crates/vox-scientia/src/manuscript/scaffold/render.rs)
- [invariant] render_imrad produces markdown with heading for each ForbiddenSection and immediately followed by TODO(narrative) block  (crates/vox-scientia/src/manuscript/scaffold/render.rs)
- [happy] Results table contains claim text, Supported status, confidence interval [0.180, 0.275], and trusty URI with nanopub address  (crates/vox-scientia/src/manuscript/scaffold/render.rs)
- [edge] When authors list is empty, render_imrad includes TODO(author) block in markdown  (crates/vox-scientia/src/manuscript/scaffold/render.rs)
- [happy] When methods_summary is provided in input, render_imrad includes the summary text in the rendered markdown  (crates/vox-scientia/src/manuscript/scaffold/render.rs)
- [happy] Introduction section contains cited facts with citation keys present in rendered markdown  (crates/vox-scientia/src/manuscript/scaffold/render.rs)
- [invariant] Each forbidden section body contains only HTML comments, blank lines, and no narrative prose  (crates/vox-scientia/src/manuscript/scaffold/render.rs)

### `build_ro_crate_json`  (edge, happy; EXTRACTED)
- [happy] When metadata with main_entity is provided, build_ro_crate_json includes a #mainEntity node in the graph with @type SoftwareSourceCode and vox:entryPoint, vox:timeoutSeconds, vox:expectedOutputs predicates  (crates/vox-scientia/src/ro_crate/metadata.rs)
- [happy] When metadata with main_entity is provided, the JSON-LD @context array includes an object entry that declares the vox: prefix  (crates/vox-scientia/src/ro_crate/metadata.rs)
- [happy] build_ro_crate_json generates a JSON-LD @graph array that contains at least one node with @type Dataset  (crates/vox-scientia/src/ro_crate/metadata.rs)
- [happy] When metadata contains a doi value, the resulting JSON-LD Dataset node includes an identifier field  (crates/vox-scientia/src/ro_crate/metadata.rs)
- [happy] When main_entity has no figures, the mainEntity node in @graph omits the vox:figures key  (crates/vox-scientia/src/ro_crate/metadata.rs)
- [happy] When main_entity contains figures, build_ro_crate_json includes vox:figures array with ImageObject entries containing path, sha3_256_hex, vox:sourceScript, vox:renderedAtMs, and vox:captionHint fields  (crates/vox-scientia/src/ro_crate/metadata.rs)
- [edge] When a figure has no caption_hint, the resulting JSON-LD object omits the vox:captionHint key  (crates/vox-scientia/src/ro_crate/metadata.rs)
- [happy] When main_entity contains multiple figures, build_ro_crate_json preserves figure order in the vox:figures array  (crates/vox-scientia/src/ro_crate/metadata.rs)
- [happy] When metadata has no main_entity, the resulting @graph does not contain a mainEntity node  (crates/vox-scientia/src/ro_crate/metadata.rs)

### `approval_for()`  (error, happy; EXTRACTED)
- [error] approval_for() returns error when no review decision exists for the claim  (crates/vox-scientia/src/review_flow.rs)
- [error] approval_for() error message references 'publication-claim-review' command when no decision  (crates/vox-scientia/src/review_flow.rs)
- [error] approval_for() returns error when latest review decision is 'rejected'  (crates/vox-scientia/src/review_flow.rs)
- [error] approval_for() error mentions 'rejected' when claim was rejected  (crates/vox-scientia/src/review_flow.rs)
- [happy] approval_for() succeeds after record_claim_review() stores an approved decision  (crates/vox-scientia/src/review_flow.rs)
- [happy] approval_for() token bound_digest() equals manifest content_sha3_256 after approval  (crates/vox-scientia/src/review_flow.rs)
- [happy] approval_for() returns a token after record_claim_review() succeeds  (crates/vox-scientia/src/review_flow.rs)
- [happy] approval_for() returns a token bound to a specific publication ID  (crates/vox-scientia/src/review_flow.rs)

### `scan`  (edge, error, happy, invariant; EXTRACTED)
- [invariant] scan produces no output when dependency consumer count is below threshold (3)  (crates/vox-scientia/src/producers/dep_adoption.rs)
- [happy] scan emits exactly one ResearchEvent with finding_id starting with algimp- and containing -dep- infix when dependency meets consumer threshold  (crates/vox-scientia/src/producers/dep_adoption.rs)
- [edge] scan returns empty collection when workspace Cargo.toml is missing  (crates/vox-scientia/src/producers/dep_adoption.rs)
- [edge] scan in doc_corpus returns empty collection for empty documentation directory  (crates/vox-scientia/src/producers/doc_corpus.rs)
- [error] scan in doc_corpus returns empty collection (not panic) when documentation directory does not exist  (crates/vox-scientia/src/producers/doc_corpus.rs)
- [invariant] scan in doc_corpus does not emit events for Markdown files with content below size threshold  (crates/vox-scientia/src/producers/doc_corpus.rs)
- [happy] scan in doc_corpus emits exactly one ResearchEvent with finding_id starting with repinf- and containing -doc- infix for Markdown files exceeding size threshold  (crates/vox-scientia/src/producers/doc_corpus.rs)

### `escape_latex`  (edge, happy; EXTRACTED)
- [happy] escape_latex returns input unchanged when it contains only ordinary text characters  (crates/vox-scientia/src/manuscript/latex/escape.rs)
- [happy] escape_latex produces correct escape sequences for all ten TeX special characters (\, {, }, $, &, #, _, %, ^, ~)  (crates/vox-scientia/src/manuscript/latex/escape.rs)
- [happy] escape_latex preserves Unicode characters unchanged in output  (crates/vox-scientia/src/manuscript/latex/escape.rs)
- [edge] escape_latex returns empty string when given empty string input  (crates/vox-scientia/src/manuscript/latex/escape.rs)
- [happy] escape_latex escapes every underscore character independently when multiple underscores are present  (crates/vox-scientia/src/manuscript/latex/escape.rs)
- [happy] escape_latex escapes percent signs when they appear in numeric contexts  (crates/vox-scientia/src/manuscript/latex/escape.rs)

### `build_queue_snapshot()`  (happy, invariant; EXTRACTED)
- [happy] QueueSnapshot.candidates.total equals the number of input candidates  (crates/vox-scientia/src/dashboard/queue.rs)
- [happy] by_class BTreeMap in CandidateSummary correctly counts candidates grouped by candidate_class  (crates/vox-scientia/src/dashboard/queue.rs)
- [happy] top_5_by_confidence returns candidates sorted by confidence descending, then by created_at_ms descending as tiebreaker  (crates/vox-scientia/src/dashboard/queue.rs)
- [invariant] top_5_by_confidence field contains at most 5 rows even when input has more candidates  (crates/vox-scientia/src/dashboard/queue.rs)
- [happy] manifests_in_reply_window in QueueSnapshot lifts publication_id values from ReplyWindowEntry input  (crates/vox-scientia/src/dashboard/queue.rs)

### `classify_subject function`  (error, happy; EXTRACTED)
- [happy] classify_subject returns Some(CommitClass::AlgorithmicImprovement) for commit messages with 'perf:' prefix  (crates/vox-scientia/src/producers/commit_graph.rs)
- [happy] classify_subject returns Some(CommitClass::ReproducibilityInfra) for commit messages with 'refactor:', 'test:', or 'tests:' prefixes  (crates/vox-scientia/src/producers/commit_graph.rs)
- [happy] classify_subject returns Some(CommitClass::AlgorithmicImprovement) for 'feat:' messages containing 'compress' keyword, but None for unrelated feat messages  (crates/vox-scientia/src/producers/commit_graph.rs)
- [happy] classify_subject returns Some(CommitClass::PolicyGovernance) for commit messages with 'policy:' or 'docs:' prefixes  (crates/vox-scientia/src/producers/commit_graph.rs)
- [error] classify_subject returns None for unrelated commit message prefixes like 'fix:' and 'chore:', and for empty input  (crates/vox-scientia/src/producers/commit_graph.rs)

### `is_adr_path()`  (edge, error, happy; EXTRACTED)
- [happy] is_adr_path recognizes 'decisions' ancestor directory with year-prefixed filenames as ADR paths  (crates/vox-scientia/src/producers/adr_emergence.rs)
- [happy] is_adr_path recognizes filenames with numbered prefixes (e.g., 0001-foo.md) as ADR paths  (crates/vox-scientia/src/producers/adr_emergence.rs)
- [error] is_adr_path rejects non-markdown files (e.g., .txt) even in adr directories  (crates/vox-scientia/src/producers/adr_emergence.rs)
- [error] is_adr_path rejects markdown files without adr/decisions ancestor or numeric prefix  (crates/vox-scientia/src/producers/adr_emergence.rs)
- [edge] is_adr_path requires dash separator between numeric prefix and filename (rejects 0001foo.md and 0001_foo.md)  (crates/vox-scientia/src/producers/adr_emergence.rs)

### `render_finding_page()`  (happy; EXTRACTED)
- [happy] Authors with ORCID IDs render as clickable links while those without render as plain text nodes  (crates/vox-scientia/src/findings_site/render.rs)
- [happy] Verified claims are rendered as anchor links containing claim ID and linked text  (crates/vox-scientia/src/findings_site/render.rs)
- [happy] HTML special characters in title and author names are escaped as entities, preventing XSS injection  (crates/vox-scientia/src/findings_site/render.rs)
- [happy] When retraction is present, render_finding_page() emits vox-retraction-banner before vox-body and includes citation_retracted meta tag  (crates/vox-scientia/src/findings_site/render.rs)
- [happy] render_finding_page() marks current version with vox-current-version class in history table  (crates/vox-scientia/src/findings_site/render.rs)

### `run_in_sandbox`  (edge, happy; EXTRACTED)
- [happy] run_in_sandbox returns output with exit_code=0 when running command 'echo ok'  (crates/vox-scientia/src/replay/sandbox.rs)
- [happy] run_in_sandbox captures stdout output from executed command  (crates/vox-scientia/src/replay/sandbox.rs)
- [happy] run_in_sandbox sets timed_out=false when command completes within timeout period  (crates/vox-scientia/src/replay/sandbox.rs)
- [happy] Captures and returns non-zero exit codes from child processes  (crates/vox-scientia/src/replay/sandbox.rs)
- [edge] Enforces timeout and kills long-running child processes, returning timed_out=true and exit_code=None  (crates/vox-scientia/src/replay/sandbox.rs)

### `truncate`  (edge, happy, invariant; EXTRACTED)
- [happy] truncate returns input unchanged when string length is within limit  (crates/vox-scientia/src/replay/mod.rs)
- [happy] truncate appends '…[truncated]' marker when string exceeds length limit  (crates/vox-scientia/src/replay/mod.rs)
- [invariant] truncate result length stays within reasonable bounds (original limit plus marker length) when string is truncated  (crates/vox-scientia/src/replay/mod.rs)
- [invariant] truncate does not panic and produces valid UTF-8 output when truncating multi-byte UTF-8 characters  (crates/vox-scientia/src/replay/mod.rs)
- [edge] truncate appends '…[truncated]' marker even when string contains multi-byte UTF-8 characters  (crates/vox-scientia/src/replay/mod.rs)

### `ComparisonOutcome.mismatches`  (edge, error, happy; EXTRACTED)
- [happy] ComparisonOutcome.mismatches is empty when all file hashes match expected values  (crates/vox-scientia/src/replay/hash_compare.rs)
- [error] ComparisonOutcome.mismatches contains one entry with expected_hex field populated when hash mismatches  (crates/vox-scientia/src/replay/hash_compare.rs)
- [edge] ComparisonOutcome.mismatches records missing files with actual_hex field set to literal string '<missing>'  (crates/vox-scientia/src/replay/hash_compare.rs)
- [happy] ComparisonOutcome.mismatches contains only entries for files with hash mismatches, not matching files  (crates/vox-scientia/src/replay/hash_compare.rs)

### `compare_output_hashes`  (error, happy; EXTRACTED)
- [happy] compare_output_hashes returns outcome with all_match=true when file content hash matches expected hash  (crates/vox-scientia/src/replay/hash_compare.rs)
- [error] compare_output_hashes returns outcome with all_match=false when file content hash does not match expected hash  (crates/vox-scientia/src/replay/hash_compare.rs)
- [error] compare_output_hashes returns Ok outcome instead of IO error when specified file does not exist  (crates/vox-scientia/src/replay/hash_compare.rs)
- [error] compare_output_hashes returns outcome with all_match=false when some but not all files have matching hashes  (crates/vox-scientia/src/replay/hash_compare.rs)

### `count_pub_symbols()`  (edge, happy, invariant; EXTRACTED)
- [happy] count_pub_symbols correctly counts pub fn, pub struct, pub enum, and pub trait items  (crates/vox-scientia/src/producers/api_surface.rs)
- [happy] count_pub_symbols ignores commented-out pub declarations in source code  (crates/vox-scientia/src/producers/api_surface.rs)
- [invariant] count_pub_symbols counts indented pub functions due to trim_start behavior (known false-positive bounded by threshold)  (crates/vox-scientia/src/producers/api_surface.rs)
- [edge] count_pub_symbols returns 0 for empty input  (crates/vox-scientia/src/producers/api_surface.rs)

### `nanopub_build()`  (error, happy; EXTRACTED)
- [happy] nanopub_build() produces Trusty URI containing RA artifact code  (crates/vox-scientia/src/review_flow.rs)
- [happy] nanopub_build() persists nanopub with published_state='local' and validated_offline=true  (crates/vox-scientia/src/review_flow.rs)
- [error] nanopub_build() returns Err when token digest differs from current manifest digest  (crates/vox-scientia/src/review_flow.rs)
- [error] nanopub_build() error message contains 'stale' or 'changed' to explain content was edited after approval  (crates/vox-scientia/src/review_flow.rs)

### `render_arxiv_bundle`  (happy, invariant; EXTRACTED)
- [happy] render_arxiv_bundle with minimal input and no figures produces a bundle containing exactly one entry with path main.tex containing LaTeX documentclass  (crates/vox-scientia/src/manuscript/latex/bundle.rs)
- [happy] render_arxiv_bundle preserves supplied figure blobs in the bundle at their declared paths alongside main.tex  (crates/vox-scientia/src/manuscript/latex/bundle.rs)
- [happy] render_arxiv_bundle produces a bundle that can be successfully read by list_bundle_entries, returning a non-empty list  (crates/vox-scientia/src/manuscript/latex/bundle.rs)
- [invariant] render_arxiv_bundle produces identical byte output for identical input (deterministic tar generation)  (crates/vox-scientia/src/manuscript/latex/bundle.rs)

### `render_latex`  (edge, happy; EXTRACTED)
- [happy] render_latex output contains longtable environment with escaped percent signs and Trusty URI links  (crates/vox-scientia/src/manuscript/latex/render.rs)
- [happy] render_latex output includes includegraphics directives and provenance comments with hash and source script metadata  (crates/vox-scientia/src/manuscript/latex/render.rs)
- [happy] render_latex output contains thebibliography environment with bibitem entries and URL links  (crates/vox-scientia/src/manuscript/latex/render.rs)
- [edge] render_latex emits 'Anonymous' placeholder and TODO comment when authors list is empty  (crates/vox-scientia/src/manuscript/latex/render.rs)

### `validate_offline()`  (error, happy; EXTRACTED)
- [happy] validate_offline() accepts valid nanopub TriG strings signed with genuine signatures  (crates/vox-scientia/tests/nanopub_conformance.rs)
- [error] validate_offline() rejects tampered nanopub TriG strings with invalid signatures  (crates/vox-scientia/tests/nanopub_conformance.rs)
- [happy] validate_offline() accepts all valid signed nanopub fixtures from the conformance set  (crates/vox-scientia/tests/nanopub_conformance.rs)
- [error] validate_offline() rejects all tampered/invalid nanopub fixtures  (crates/vox-scientia/tests/nanopub_conformance.rs)

### `AiDisclosureBlock::build`  (happy, invariant; EXTRACTED)
- [happy] When built with empty tool list, disclosure_text contains "No AI tools" and both human_author_accountable and no_llm_generated_figures are true  (crates/vox-scientia/src/ro_crate/ai_disclosure.rs)
- [happy] When built with tool list, the disclosure_text contains the tool name (Claude)  (crates/vox-scientia/src/ro_crate/ai_disclosure.rs)
- [invariant] Disclosure always sets no_llm_generated_figures to true regardless of input  (crates/vox-scientia/src/ro_crate/ai_disclosure.rs)

### `AtomicClaim`  (happy, invariant; EXTRACTED)
- [invariant] Each decomposed claim has non-empty text field  (crates/vox-scientia/src/claim_extractor/atomic.rs)
- [happy] AtomicClaim serializes to JSON and deserializes back with matching id field  (crates/vox-scientia/src/claim_extractor/types.rs)
- [invariant] AtomicClaim preserves verifiability field through JSON round-trip  (crates/vox-scientia/src/claim_extractor/types.rs)

### `ChronoFilter::filter_hits()`  (happy, invariant; EXTRACTED)
- [happy] Evidence hits with years after the claim year are filtered out  (crates/vox-scientia/src/inspect_bridge/chronofact.rs)
- [happy] Evidence hits with years before the claim year are retained  (crates/vox-scientia/src/inspect_bridge/chronofact.rs)
- [invariant] Evidence hits from the same year as the claim are filtered out (strict less-than prior art check)  (crates/vox-scientia/src/inspect_bridge/chronofact.rs)

### `EvidenceConflictDetector::detect`  (happy, invariant; EXTRACTED)
- [happy] Returns None when all PolarizedHits have the same polarity (all supporting)  (crates/vox-scientia/src/inspect_bridge/conflict.rs)
- [happy] Returns Some(Conflict) with equal supporting and contradicting hit counts when hits have opposing polarities, and calculates conflict_score correctly  (crates/vox-scientia/src/inspect_bridge/conflict.rs)
- [invariant] Ignores hits below the similarity threshold (0.8) when detecting conflicts, returning None  (crates/vox-scientia/src/inspect_bridge/conflict.rs)

### `EvidenceConflictDetector::detect()`  (edge, happy; EXTRACTED)
- [happy] No conflict is detected when all evidence hits have supporting polarity  (crates/vox-scientia/src/inspect_bridge/conflict.rs)
- [happy] Conflict is detected with correct supporting/contradicting hit counts and conflict score when opposing polarities exist  (crates/vox-scientia/src/inspect_bridge/conflict.rs)
- [edge] Evidence hits below the similarity threshold (0.8) are excluded from conflict detection  (crates/vox-scientia/src/inspect_bridge/conflict.rs)

### `TopComplianceReport::assess`  (happy; EXTRACTED)
- [happy] When all assets are false (no_data, no_code, no_docs), overall_level is Level0 and is_level2_or_above() returns false  (crates/vox-scientia/src/ro_crate/compliance.rs)
- [happy] When data and code are present but docs absent, is_level2_or_above() returns true  (crates/vox-scientia/src/ro_crate/compliance.rs)
- [happy] When all assets present (data, code, and docs), overall_level is Level3  (crates/vox-scientia/src/ro_crate/compliance.rs)

### `dedup_finding_candidates function`  (edge, happy; EXTRACTED)
- [happy] dedup_finding_candidates collapses duplicate finding IDs, returning exactly 2 elements when given a vector with 3 items where 2 are duplicates  (crates/vox-scientia/src/producers/dedup.rs)
- [happy] dedup_finding_candidates preserves the order of first occurrence of finding IDs, returning [a, b, c] when given [a, b, a, c]  (crates/vox-scientia/src/producers/dedup.rs)
- [edge] dedup_finding_candidates returns an empty vector when given an empty input vector  (crates/vox-scientia/src/producers/dedup.rs)

### `extract_workspace_dep_names`  (happy; EXTRACTED)
- [happy] extract_workspace_dep_names correctly extracts dependency names (serde, tokio, anyhow) from workspace.dependencies section in TOML  (crates/vox-scientia/src/producers/dep_adoption.rs)
- [happy] extract_workspace_dep_names lowercases all extracted dependency names (Foo, BAR become foo, bar)  (crates/vox-scientia/src/producers/dep_adoption.rs)
- [happy] extract_workspace_dep_names returns empty collection when dependencies are not in workspace.dependencies section  (crates/vox-scientia/src/producers/dep_adoption.rs)

### `format_iso_date`  (happy; EXTRACTED)
- [happy] format_iso_date(0) returns the ISO 8601 date string 1970-01-01  (crates/vox-scientia/src/ro_crate/metadata.rs)
- [happy] format_iso_date(86_400) returns the ISO 8601 date string 1970-01-02 (one day later)  (crates/vox-scientia/src/ro_crate/metadata.rs)
- [happy] format_iso_date(1_700_006_400) correctly converts Unix timestamp to ISO 8601 date 2023-11-15  (crates/vox-scientia/src/ro_crate/metadata.rs)

### `is_section_forbidden`  (edge, happy; EXTRACTED)
- [happy] is_section_forbidden returns true for known forbidden section titles regardless of case or surrounding whitespace  (crates/vox-scientia/src/manuscript/scaffold/safe_slots.rs)
- [happy] is_section_forbidden returns false for safe section names (Methods, Results, Limitations, References, Author Block)  (crates/vox-scientia/src/manuscript/scaffold/safe_slots.rs)
- [edge] is_section_forbidden returns false for empty string and whitespace-only input  (crates/vox-scientia/src/manuscript/scaffold/safe_slots.rs)

### `parse_suggestions()`  (edge, happy; EXTRACTED)
- [happy] parse_suggestions() correctly extracts JSON suggestions from fenced code blocks  (crates/vox-scientia/src/evidence_assist.rs)
- [edge] parse_suggestions() returns empty array for non-JSON input, not error  (crates/vox-scientia/src/evidence_assist.rs)
- [edge] parse_suggestions() accepts and parses suggestions with unknown 'kind' values  (crates/vox-scientia/src/evidence_assist.rs)

### `record_claim_review()`  (error, happy; EXTRACTED)
- [happy] record_claim_review() persists bound_digest matching publication manifest's content_sha3_256  (crates/vox-scientia/src/review_flow.rs)
- [happy] record_claim_review() stores decision, actor, and reason parameters in ReviewDecisionRow  (crates/vox-scientia/src/review_flow.rs)
- [error] record_claim_review() returns error when publication manifest does not exist  (crates/vox-scientia/src/review_flow.rs)

### `scan function`  (edge, happy; EXTRACTED)
- [edge] scan returns an empty vector when a crate has 3 public symbols, which is below the emission threshold  (crates/vox-scientia/src/producers/api_surface.rs)
- [happy] scan emits a single FindingCandidateProposed event with finding_id starting with 'algimp-' and containing '-api-' when a crate has 20 public symbols (above threshold)  (crates/vox-scientia/src/producers/api_surface.rs)
- [edge] scan returns an empty vector when scanning a crate with only main.rs (bin-only, no lib.rs)  (crates/vox-scientia/src/producers/api_surface.rs)

### `scan()`  (edge, happy; EXTRACTED)
- [edge] scan returns empty result when docs directory is missing  (crates/vox-scientia/src/producers/adr_emergence.rs)
- [happy] scan emits FindingCandidateProposed event with polgov and adr identifiers for valid ADR files  (crates/vox-scientia/src/producers/adr_emergence.rs)
- [happy] scan ignores markdown files in docs that are not ADRs (e.g., docs/src/guide.md)  (crates/vox-scientia/src/producers/adr_emergence.rs)

### `validate_offline`  (error, happy; EXTRACTED)
- [happy] when validating a properly signed TriG offline, validation succeeds  (crates/vox-scientia/src/nanopub/spec.rs)
- [happy] when validating an enriched nanopub with claim metadata offline, validation succeeds  (crates/vox-scientia/src/nanopub/spec.rs)
- [error] when validating tampered TriG content with modified assertion text, validation fails with an error  (crates/vox-scientia/src/nanopub/spec.rs)

### `PublicationApprovalToken`  (happy; EXTRACTED)
- [happy] Token has bound_digest() method that matches the manifest's content_sha3_256  (crates/vox-scientia/src/review_flow.rs)
- [happy] Token has claim_id() method that matches the claim ID  (crates/vox-scientia/src/review_flow.rs)

### `ReplayOutcome::NonZeroExit`  (happy; EXTRACTED)
- [happy] ReplayOutcome::NonZeroExit serializes to JSON with 'kind' field containing string 'non_zero_exit'  (crates/vox-scientia/src/replay/report.rs)
- [happy] ReplayOutcome::NonZeroExit includes exit_code value in JSON serialization  (crates/vox-scientia/src/replay/report.rs)

### `SpanChecker::check()`  (error, happy; EXTRACTED)
- [happy] check() returns true when span coordinates are within source string bounds  (crates/vox-scientia/src/claim_extractor/span.rs)
- [error] check() returns false when span coordinates exceed source string length  (crates/vox-scientia/src/claim_extractor/span.rs)

### `build_prompt()`  (happy; EXTRACTED)
- [happy] build_prompt() returns messages with system message first and user message second  (crates/vox-scientia/src/evidence_assist.rs)
- [happy] build_prompt() user message contains the claim text, verdict, and formatted confidence (to 2 decimals)  (crates/vox-scientia/src/evidence_assist.rs)

### `builtin_class_defaults`  (invariant; EXTRACTED)
- [invariant] all micro-class findings have zero negative_result_quota in builtin defaults  (crates/vox-scientia/src/class_routing/defaults.rs)
- [invariant] AlgorithmicImprovement and ReproducibilityInfra have critic_allowed=true; policy-implication classes have critic_allowed=false  (crates/vox-scientia/src/class_routing/defaults.rs)

### `load_class_defaults_from_yaml`  (error, happy; EXTRACTED)
- [happy] YAML with valid class policies parses successfully and preserves reply_window_days, critic_allowed, and recommended_venues fields  (crates/vox-scientia/src/class_routing/defaults.rs)
- [error] malformed YAML input returns ClassRoutingError::Yaml  (crates/vox-scientia/src/class_routing/defaults.rs)

### `publish_stub`  (happy; EXTRACTED)
- [happy] Returns PublishResult with success=false, no nanopub_uri, and error message 'Phase 8 stub'  (crates/vox-scientia/src/nanopub/network.rs)
- [happy] when publishing via stub implementation, returns a result with success=false, nanopub_uri=None, and error containing "Phase 8 stub"  (crates/vox-scientia/src/nanopub/network.rs)

### `recommended_venues_for`  (happy; EXTRACTED)
- [happy] AlgorithmicImprovement class routes to ICSE and FSE venues; does not route to IMC  (crates/vox-scientia/src/class_routing/routing.rs)
- [happy] both ModelCapabilityAtlas and ProviderReliabilityAtlas route to either IMC or MLSys venues  (crates/vox-scientia/src/class_routing/routing.rs)

### `resolve_or_create_identity()`  (happy; EXTRACTED)
- [happy] resolve_or_create_identity() reuses stored RSA private key across multiple calls with same user  (crates/vox-scientia/src/review_flow.rs)
- [happy] resolve_or_create_identity() persists ORCID from first call and reuses it when omitted on second call  (crates/vox-scientia/src/review_flow.rs)

### `validate_claim_envelope()`  (error, happy; EXTRACTED)
- [happy] validate_claim_envelope() accepts envelopes with required fields: id, text, verifiability, verifiability_score  (crates/vox-scientia/src/claim_extractor/constrained.rs)
- [error] validate_claim_envelope() rejects envelopes missing required fields  (crates/vox-scientia/src/claim_extractor/constrained.rs)

### `AtomicDecomposer::decompose()`  (happy; EXTRACTED)
- [happy] decompose() returns non-empty vector of claims  (crates/vox-scientia/src/claim_extractor/atomic.rs)

### `CostRollup`  (happy; EXTRACTED)
- [happy] CostRollup serializes to JSON and deserializes back to an equal value  (crates/vox-scientia/src/dashboard/cost.rs)

### `FindingClass::from_str`  (happy; EXTRACTED)
- [happy] all builtin FindingClass variants parse correctly from their string representations and unknown strings return None  (crates/vox-scientia/src/class_routing/defaults.rs)

### `FindingClass::is_atlas`  (invariant; EXTRACTED)
- [invariant] ModelCapabilityAtlas and ProviderReliabilityAtlas return true; non-atlas classes return false  (crates/vox-scientia/src/class_routing/defaults.rs)

### `InspectTaskDescriptor::add_sample`  (happy; EXTRACTED)
- [happy] Increments sample_count each time a sample is added  (crates/vox-scientia/src/inspect_bridge/inspect_task.rs)

### `InspectTaskDescriptor::add_sample()`  (happy; EXTRACTED)
- [happy] Adding samples to a task increments the sample count accordingly  (crates/vox-scientia/src/inspect_bridge/inspect_task.rs)

### `InspectTaskDescriptor::new`  (happy; EXTRACTED)
- [happy] Initializes a new task with zero samples  (crates/vox-scientia/src/inspect_bridge/inspect_task.rs)

### `InspectTaskDescriptor::sample_count()`  (happy; EXTRACTED)
- [happy] Newly constructed task instances have zero samples  (crates/vox-scientia/src/inspect_bridge/inspect_task.rs)

### `InspectTaskDescriptor::to_json`  (happy; EXTRACTED)
- [happy] Serializes task to JSON containing task_id field and samples array with correct input values  (crates/vox-scientia/src/inspect_bridge/inspect_task.rs)

### `InspectTaskDescriptor::to_json()`  (happy; EXTRACTED)
- [happy] Task serialization to JSON preserves task ID and all sample data with correct field values  (crates/vox-scientia/src/inspect_bridge/inspect_task.rs)

### `MiniCheckVerifier::verify_claim()`  (happy; EXTRACTED)
- [happy] verify_claim() returns a result with support_score field bounded between 0.0 and 1.0  (crates/vox-scientia/src/claim_extractor/minicheck.rs)

### `QueueSnapshot`  (happy; EXTRACTED)
- [happy] QueueSnapshot JSON serialization contains all documented top-level keys: candidates, claims_pending, manifests_in_reply_window, retraction_queue, stalls  (crates/vox-scientia/src/dashboard/queue.rs)

### `ReplayOutcome::Pass`  (happy; EXTRACTED)
- [happy] ReplayOutcome::Pass serializes to JSON with 'kind' field containing string 'pass'  (crates/vox-scientia/src/replay/report.rs)

### `ReplayReport`  (invariant; EXTRACTED)
- [invariant] ReplayReport survives round-trip through JSON serialization/deserialization with equality preserved  (crates/vox-scientia/src/replay/report.rs)

### `ReviewDecisionLike`  (happy; EXTRACTED)
- [happy] Trait impl for ReviewDecisionRow correctly routes claim_id, bound_digest, and decision accessors; mint_from_decision works with ReviewDecisionRow  (crates/vox-scientia/src/review/mod.rs)

### `SpanBound`  (invariant; EXTRACTED)
- [invariant] Each decomposed claim has span.end > span.start  (crates/vox-scientia/src/claim_extractor/atomic.rs)

### `VenueCriticPolicy::Forbidden`  (happy; EXTRACTED)
- [happy] VenueCriticPolicy::Forbidden.allows_critic() returns false  (crates/vox-scientia/src/critic_gate/venue.rs)

### `VeriScoreGate::score_sentence`  (happy; EXTRACTED)
- [happy] scoring a numeric claim returns a score >= 0.7 and assigns a non-Unverifiable class  (crates/vox-scientia/src/claim_extractor/veriscore.rs)

### `assertion_ttl_for_claim`  (happy; EXTRACTED)
- [happy] when building enriched assertion TTL with claim metadata, the result contains scientia:text, scientia:relation, scientia:confidence, scientia:noveltyVerdict, and closestPriorArt properties  (crates/vox-scientia/src/nanopub/spec.rs)

### `atlas_gate_applies_to`  (invariant; EXTRACTED)
- [invariant] atlas classes return true; non-atlas classes (AlgorithmicImprovement, ReproducibilityInfra, PolicyGovernance, TelemetryTrust, Other) return false  (crates/vox-scientia/src/class_routing/routing.rs)

### `build_and_sign`  (happy; EXTRACTED)
- [happy] when building and signing a nanopub, produces a trusty_uri that is non-empty and contains the RA artifact code  (crates/vox-scientia/src/nanopub/spec.rs)

### `build_cff_json`  (happy; EXTRACTED)
- [happy] When author has ORCID set, build_cff_json includes the ORCID value in the authors array JSON  (crates/vox-scientia/src/ro_crate/cff.rs)

### `build_cost_rollup()`  (happy; EXTRACTED)
- [happy] by_provider field in returned CostRollup preserves order and exact values from input  (crates/vox-scientia/src/dashboard/cost.rs)

### `build_highwire_meta_tags()`  (happy; EXTRACTED)
- [happy] Highwire meta tags copy citation_title, citation_author, and citation_publication_date from FindingPage fields  (crates/vox-scientia/src/findings_site/meta.rs)

### `count_scientia_nanopubs_for_claim()`  (invariant; EXTRACTED)
- [invariant] count_scientia_nanopubs_for_claim() returns 0 when nanopub_build() is refused, indicating no persistence occurred  (crates/vox-scientia/src/review_flow.rs)

### `dedup_finding_candidates`  (happy; EXTRACTED)
- [happy] dedup_finding_candidates returns empty collection when given empty vector input  (crates/vox-scientia/src/producers/dedup.rs)

### `effective_orcid()`  (happy; EXTRACTED)
- [happy] effective_orcid() uses row ORCID when param ORCID is None  (crates/vox-scientia/src/review_flow.rs)

### `evaluate_gate`  (happy; EXTRACTED)
- [happy] gate clears and returns GateReason::TwoHumans when two distinct human approvers are present  (crates/vox-scientia/src/critic_gate/gate.rs)

### `is_adr_path`  (happy; EXTRACTED)
- [happy] when checking paths with ancestor directories named 'adr' or 'decisions', returns true  (crates/vox-scientia/src/producers/adr_emergence.rs)

### `list_bundle_entries`  (happy; EXTRACTED)
- [happy] list_bundle_entries on a bundle with multiple figures returns an entry count equal to the number of figures plus main.tex  (crates/vox-scientia/src/manuscript/latex/bundle.rs)

### `p95`  (happy; EXTRACTED)
- [happy] p95 function returns None when given empty slice  (crates/vox-scientia/src/producers/heuristics.rs)

### `parse_main_entity_from_json`  (error; EXTRACTED)
- [error] parse_main_entity_from_json returns ParseError::Schema variant when input JSON lacks required @graph field  (crates/vox-scientia/src/replay/contract.rs)

### `publication_session_id()`  (invariant; EXTRACTED)
- [invariant] publication_session_id() returns identical output for identical input and different output for different input  (crates/vox-scientia/src/review_flow.rs)

### `reply_window_days_for, negative_result_quota_for, critic_allowed_for, recommended_venues_for`  (edge; EXTRACTED)
- [edge] when class defaults are empty, all lookup functions return safe fallback values: 14 days window, 0 quota, false for critic allowed, empty venue list  (crates/vox-scientia/src/class_routing/routing.rs)

### `review_flow module`  (invariant; EXTRACTED)
- [invariant] review_flow source code does not contain publish_to_network or use_test_server symbols  (crates/vox-scientia/src/review_flow.rs)

### `scan_commits function`  (edge; EXTRACTED)
- [edge] scan_commits returns an empty vector when the repository path does not exist  (crates/vox-scientia/src/producers/commit_graph.rs)

### `token`  (happy; EXTRACTED)
- [happy] Token accessors (claim_id, bound_digest) return values matching source decision fields  (crates/vox-scientia/src/review/mod.rs)

### `write_lib_rs function`  (happy; EXTRACTED)
- [happy] write_lib_rs creates a directory structure with a lib.rs file containing generated public function definitions at the expected file path in src/  (crates/vox-scientia/src/producers/api_surface.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`AtomicDecomposer::decompose()`** — only: _decompose() returns non-empty vector of claims_
- **`CostRollup`** — only: _CostRollup serializes to JSON and deserializes back to an equal value_
- **`FindingClass::from_str`** — only: _all builtin FindingClass variants parse correctly from their string representations and unknown strings return None_
- **`InspectTaskDescriptor::add_sample`** — only: _Increments sample_count each time a sample is added_
- **`InspectTaskDescriptor::add_sample()`** — only: _Adding samples to a task increments the sample count accordingly_
- **`InspectTaskDescriptor::new`** — only: _Initializes a new task with zero samples_
- **`InspectTaskDescriptor::sample_count()`** — only: _Newly constructed task instances have zero samples_
- **`InspectTaskDescriptor::to_json`** — only: _Serializes task to JSON containing task_id field and samples array with correct input values_
- **`InspectTaskDescriptor::to_json()`** — only: _Task serialization to JSON preserves task ID and all sample data with correct field values_
- **`MiniCheckVerifier::verify_claim()`** — only: _verify_claim() returns a result with support_score field bounded between 0.0 and 1.0_
- **`PublicationApprovalToken`** — only: _Token has bound_digest() method that matches the manifest's content_sha3_256_
- **`QueueSnapshot`** — only: _QueueSnapshot JSON serialization contains all documented top-level keys: candidates, claims_pending, manifests_in_reply_window, retraction_queue, stalls_
- **`ReplayOutcome::NonZeroExit`** — only: _ReplayOutcome::NonZeroExit serializes to JSON with 'kind' field containing string 'non_zero_exit'_
- **`ReplayOutcome::Pass`** — only: _ReplayOutcome::Pass serializes to JSON with 'kind' field containing string 'pass'_
- **`ReviewDecisionLike`** — only: _Trait impl for ReviewDecisionRow correctly routes claim_id, bound_digest, and decision accessors; mint_from_decision works with ReviewDecisionRow_
- **`TopComplianceReport::assess`** — only: _When all assets are false (no_data, no_code, no_docs), overall_level is Level0 and is_level2_or_above() returns false_
- **`VenueCriticPolicy::Forbidden`** — only: _VenueCriticPolicy::Forbidden.allows_critic() returns false_
- **`VeriScoreGate::score_sentence`** — only: _scoring a numeric claim returns a score >= 0.7 and assigns a non-Unverifiable class_
- **`assertion_ttl_for_claim`** — only: _when building enriched assertion TTL with claim metadata, the result contains scientia:text, scientia:relation, scientia:confidence, scientia:noveltyVerdict, and closestPriorArt properties_
- **`build_and_sign`** — only: _when building and signing a nanopub, produces a trusty_uri that is non-empty and contains the RA artifact code_
- **`build_cff_json`** — only: _When author has ORCID set, build_cff_json includes the ORCID value in the authors array JSON_
- **`build_cost_rollup()`** — only: _by_provider field in returned CostRollup preserves order and exact values from input_
- **`build_highwire_meta_tags()`** — only: _Highwire meta tags copy citation_title, citation_author, and citation_publication_date from FindingPage fields_
- **`build_prompt()`** — only: _build_prompt() returns messages with system message first and user message second_
- **`dedup_finding_candidates`** — only: _dedup_finding_candidates returns empty collection when given empty vector input_
- **`effective_orcid()`** — only: _effective_orcid() uses row ORCID when param ORCID is None_
- **`evaluate_gate`** — only: _gate clears and returns GateReason::TwoHumans when two distinct human approvers are present_
- **`extract_workspace_dep_names`** — only: _extract_workspace_dep_names correctly extracts dependency names (serde, tokio, anyhow) from workspace.dependencies section in TOML_
- **`format_iso_date`** — only: _format_iso_date(0) returns the ISO 8601 date string 1970-01-01_
- **`is_adr_path`** — only: _when checking paths with ancestor directories named 'adr' or 'decisions', returns true_
- **`list_bundle_entries`** — only: _list_bundle_entries on a bundle with multiple figures returns an entry count equal to the number of figures plus main.tex_
- **`p95`** — only: _p95 function returns None when given empty slice_
- **`publish_stub`** — only: _Returns PublishResult with success=false, no nanopub_uri, and error message 'Phase 8 stub'_
- **`recommended_venues_for`** — only: _AlgorithmicImprovement class routes to ICSE and FSE venues; does not route to IMC_
- **`render_finding_page()`** — only: _Authors with ORCID IDs render as clickable links while those without render as plain text nodes_
- **`resolve_or_create_identity()`** — only: _resolve_or_create_identity() reuses stored RSA private key across multiple calls with same user_
- **`token`** — only: _Token accessors (claim_id, bound_digest) return values matching source decision fields_
- **`write_lib_rs function`** — only: _write_lib_rs creates a directory structure with a lib.rs file containing generated public function definitions at the expected file path in src/_
