# Semantic Behavior Map — `vox-publisher`

Deterministically synthesized from 202 distinct proven-behavior claims (of 204 extracted) across 50 symbols. 13 symbols have an explicit error-path proof; **24 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `WorthinessDecision`  (edge, error, happy, invariant; EXTRACTED)
- [happy] evaluate_worthiness() returns WorthinessDecision::Publish when all inputs meet publish-ready thresholds  (crates/vox-publisher/src/publication_worthiness.rs)
- [error] evaluate_worthiness() returns AbstainDoNotPublish when any red_line_violation_ids are set  (crates/vox-publisher/src/publication_worthiness.rs)
- [error] evaluate_worthiness() returns AbstainDoNotPublish when epistemic, reproducibility, novelty, reliability, or metadata_policy scores are below threshold (0.1)  (crates/vox-publisher/src/publication_worthiness.rs)
- [edge] evaluate_worthiness() returns AskForEvidence when claim_evidence_coverage falls below minimum floor (0.5)  (crates/vox-publisher/src/publication_worthiness.rs)
- [edge] evaluate_worthiness() returns AskForEvidence when meaningful_advance is false  (crates/vox-publisher/src/publication_worthiness.rs)
- [invariant] heuristic decision is not Publish without meaningful_advance evidence; remains Some even under DoubleBlind  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [happy] evaluate_worthiness() returns Publish decision when hard_metrics_ok=true, score above publish_score_min, and meaningful_advance=true  (crates/vox-publisher/src/publication_worthiness.rs)
- [error] evaluate_worthiness() returns AbstainDoNotPublish when any enabled red_line_violation_ids is present  (crates/vox-publisher/src/publication_worthiness.rs)
- [error] evaluate_worthiness() returns AbstainDoNotPublish when aggregate worthiness score is below abstain_score_max threshold  (crates/vox-publisher/src/publication_worthiness.rs)
- [error] evaluate_worthiness() returns AskForEvidence when hard_metrics_ok returns false due to metric below floor (e.g., claim_evidence_coverage < minimum)  (crates/vox-publisher/src/publication_worthiness.rs)
- [edge] evaluate_worthiness() returns AskForEvidence when meaningful_advance=false, even if hard_metrics_ok and score >= publish_score_min  (crates/vox-publisher/src/publication_worthiness.rs)
- [edge] WorthinessDecision can be non-Publish when meaningful_advance heuristic is not satisfied.  (crates/vox-publisher/src/publication_preflight/tests.rs)
- … +8 more claims

### `evaluate_worthiness()`  (edge, error, happy, invariant; EXTRACTED)
- [happy] evaluate_worthiness returns decision=WorthinessDecision::Publish for inputs with high metrics and no red-line violations  (crates/vox-publisher/src/publication_worthiness.rs)
- [error] evaluate_worthiness returns decision=WorthinessDecision::AbstainDoNotPublish when red_line_violation_ids contains any violation  (crates/vox-publisher/src/publication_worthiness.rs)
- [error] evaluate_worthiness returns decision=WorthinessDecision::AbstainDoNotPublish when core metrics (epistemic, reproducibility, novelty, reliability, metadata_policy) fall below thresholds  (crates/vox-publisher/src/publication_worthiness.rs)
- [edge] evaluate_worthiness returns decision=WorthinessDecision::AskForEvidence when claim_evidence_coverage falls below contract floor  (crates/vox-publisher/src/publication_worthiness.rs)
- [edge] evaluate_worthiness returns decision=WorthinessDecision::AskForEvidence when meaningful_advance is false  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] Returns Publish decision when contract and inputs are well-formed with high metrics  (crates/vox-publisher/src/publication_worthiness.rs)
- [invariant] Measured replayability of 0.0 causes hard_metrics_ok to fail even when declared value passes  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] Returns Publish decision when both declared and measured replayability pass thresholds  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] When given a sample contract and publish-ready inputs, evaluate_worthiness returns WorthinessDecision::Publish  (crates/vox-publisher/src/publication_worthiness.rs)
- [error] When red_line_violation_ids is non-empty with an enabled violation, evaluate_worthiness returns WorthinessDecision::AbstainDoNotPublish  (crates/vox-publisher/src/publication_worthiness.rs)
- [edge] When aggregate worthiness score is below abstain_score_max threshold, evaluate_worthiness returns WorthinessDecision::AbstainDoNotPublish  (crates/vox-publisher/src/publication_worthiness.rs)
- [edge] When claim_evidence_coverage falls below the minimum floor threshold, evaluate_worthiness returns WorthinessDecision::AskForEvidence  (crates/vox-publisher/src/publication_worthiness.rs)
- … +4 more claims

### `PeerReviewGateError`  (error; EXTRACTED)
- [error] PeerReviewGate::check() returns InsufficientApprovals error when given only one approval (below threshold)  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] PeerReviewGate::check() returns Rejected error when any review is a rejection  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] PeerReviewGate::check() returns DigestMismatch error when a review's publication_digest differs from the expected digest  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] PeerReviewGate::check() returns InvalidSignature error when a review has a tampered or invalid signature  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] gate.check() returns InsufficientApprovals error when only one review approval exists against the default requirement of 2  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] gate.check() returns Rejected error when any review has decision=Reject, regardless of approval count  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] gate.check() returns DigestMismatch error when a review's publication_digest does not match the expected digest  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] gate.check() returns InvalidSignature error when a review's signature_hex does not match the canonical sha3_hex of its payload  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] PeerReviewGate.check() returns InsufficientApprovals error when only one approval is provided  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] PeerReviewGate.check() returns Rejected error when any review is a rejection  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] PeerReviewGate.check() returns DigestMismatch error when publication_digest does not match expected digest  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] PeerReviewGate.check() returns InvalidSignature error when signature_hex is tampered  (crates/vox-publisher/src/peer_review_gate.rs)

### `build_scientia_metadata_json()`  (error, happy, invariant; EXTRACTED)
- [happy] build_scientia_metadata_json() returns Ok with scientific metadata when given title, scientific author with ORCID, and Apache-2.0 license  (crates/vox-publisher/src/citation_cff.rs)
- [happy] build_scientia_metadata_json() successfully builds metadata JSON from scientific publication metadata  (crates/vox-publisher/src/citation_cff.rs)
- [happy] build_scientia_metadata_json() successfully processes scientific metadata with license and author information  (crates/vox-publisher/src/crossref_metadata.rs)
- [happy] returns ok result and readiness_score >= 80 when author and license are properly aligned  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [error] detects author primary mismatch when metadata author differs from manifest author  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [happy] passes MetadataComplete profile validation when fully populated with authors, license, and citations  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [happy] When scientific metadata is correctly built and aligned with manifest author, preflight validation passes with readiness score >= 80.  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [error] When scientific metadata author differs from manifest author, preflight detects author_primary_mismatch error.  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [happy] When build_scientia_metadata_json() creates complete metadata, MetadataComplete profile validation succeeds.  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [happy] build_scientia_metadata_json() includes scientific block with authors array and preserves prepared_by identifier  (crates/vox-publisher/src/scientific_metadata.rs)
- [invariant] build_scientia_metadata_json() changes manifest content hash when scientific metadata block is modified  (crates/vox-publisher/src/scientific_metadata.rs)

### `crossref_work_export_json()`  (happy, invariant; EXTRACTED)
- [happy] crossref_work_export_json() generates JSON with schema vox.crossref_work_metadata.v1  (crates/vox-publisher/src/crossref_metadata.rs)
- [happy] crossref_work_export_json() includes manifest title in output JSON title field  (crates/vox-publisher/src/crossref_metadata.rs)
- [happy] crossref_work_export_json() maps Apache-2.0 SPDX license to correct URL in license.url field  (crates/vox-publisher/src/crossref_metadata.rs)
- [happy] crossref_work_export_json() includes scientific metadata contributors in output as contributors array  (crates/vox-publisher/src/crossref_metadata.rs)
- [happy] crossref_work_export_json() outputs valid schema with title, license URL, and contributor information  (crates/vox-publisher/src/crossref_metadata.rs)
- [happy] Export JSON includes correct title from manifest  (crates/vox-publisher/src/crossref_metadata.rs)
- [happy] Export JSON includes Apache 2.0 license URL when SPDX license is Apache-2.0  (crates/vox-publisher/src/crossref_metadata.rs)
- [happy] Export JSON includes contributor names from metadata  (crates/vox-publisher/src/crossref_metadata.rs)
- [invariant] Export JSON schema is vox.crossref_work_metadata.v1  (crates/vox-publisher/src/crossref_metadata.rs)

### `PeerReviewGate::check()`  (error; EXTRACTED)
- [error] PeerReviewGate::check() returns InsufficientApprovals error when fewer than required reviews approve  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] PeerReviewGate::check() returns Rejected error when any review contains a rejection  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] PeerReviewGate::check() returns DigestMismatch error when review digest differs from expected  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] Gate rejects with InsufficientApprovals error when only one approval present  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] Gate rejects with Rejected error when one review rejects  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] Gate rejects with DigestMismatch error when review digest doesn't match publication digest  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] Gate rejects with InvalidSignature error when signature is tampered  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] Returns InvalidSignature error variant when review signature_hex is tampered with  (crates/vox-publisher/src/peer_review_gate.rs)

### `effective_replayability()`  (happy; EXTRACTED)
- [happy] effective_replayability() returns the declared artifact_replayability value when artifact_replayability_measured is None  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] effective_replayability() returns the measured artifact_replayability value (ignoring declared) when artifact_replayability_measured is Some  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] effective_replayability() returns declared artifact_replayability value when artifact_replayability_measured is None  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] effective_replayability() returns artifact_replayability (declared) when artifact_replayability_measured is None  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] effective_replayability() returns artifact_replayability_measured value and ignores declared value when measured is Some  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] Returns declared artifact_replayability when artifact_replayability_measured is None  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] Returns measured value when artifact_replayability_measured is Some  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] effective_replayability() returns measured value when artifact_replayability_measured is Some  (crates/vox-publisher/src/publication_worthiness.rs)

### `clamp_text()`  (happy, invariant; EXTRACTED)
- [happy] clamp_text() preserves strings shorter than the clamping limit unchanged  (crates/vox-publisher/src/contract.rs)
- [happy] clamp_text() truncates strings exceeding the limit and appends ellipsis (...)  (crates/vox-publisher/src/contract.rs)
- [invariant] clamp_text() output character count never exceeds the specified limit  (crates/vox-publisher/src/contract.rs)
- [happy] clamp_text() returns text unchanged when input is shorter than the character limit  (crates/vox-publisher/src/contract.rs)
- [happy] clamp_text() truncates text exceeding limit, appends ellipsis, and respects max character count  (crates/vox-publisher/src/contract.rs)
- [happy] When text exceeds the character limit, clamp_text truncates and appends ellipsis  (crates/vox-publisher/src/contract.rs)
- [happy] Result fits within specified character limit  (crates/vox-publisher/src/contract.rs)

### `compile_for_publish()`  (happy, invariant; EXTRACTED)
- [invariant] compile_for_publish() generates deterministic digest for same input  (crates/vox-publisher/src/distribution_compile.rs)
- [happy] compile_for_publish() produces one channel plan when Twitter is configured  (crates/vox-publisher/src/distribution_compile.rs)
- [invariant] compile_for_publish() produces deterministic derivation digests across multiple invocations  (crates/vox-publisher/src/distribution_compile.rs)
- [happy] compile_for_publish() generates correct channel plans with matching channel and projection profile ID  (crates/vox-publisher/src/distribution_compile.rs)
- [invariant] Derivation digest is deterministic across multiple invocations  (crates/vox-publisher/src/distribution_compile.rs)
- [happy] When twitter channel configured, exactly one channel plan is emitted  (crates/vox-publisher/src/distribution_compile.rs)
- [happy] Twitter channel plan includes correct projection profile ID  (crates/vox-publisher/src/distribution_compile.rs)

### `run_preflight()`  (edge, error, happy; EXTRACTED)
- [edge] surfaces manual_required finding with code 'legacy_syndication_metadata_key' when legacy metadata key is present  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [error] fails with 'double_blind_email_in_body' finding when DoubleBlind profile detects email address in body  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [error] fails with 'double_blind_orcid_url_in_body' finding when DoubleBlind profile detects ORCID URL  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [error] fails with 'scientific_metadata_required' finding in MetadataComplete profile when scientific metadata absent  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [error] ArxivAssist profile requires abstract but not scientific_metadata_required; fails with 'abstract_required' only  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [happy] surfaces next_actions with codes 'run_default_scholarly_pipeline', 'simulate_social_routing', and 'dry_run_social_publish'  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [happy] run_preflight() generates next_actions including run_default_scholarly_pipeline, simulate_social_routing, and dry_run_social_publish for properly configured publications.  (crates/vox-publisher/src/publication_preflight/tests.rs)

### `unified_news_item_from_manifest_parts()`  (happy, invariant; EXTRACTED)
- [happy] distribution_policy.dry_run=true overrides syndication.dry_run=false to enable dry_run mode  (crates/vox-publisher/src/switching.rs)
- [happy] parses RFC3339-formatted published_at timestamp correctly  (crates/vox-publisher/src/switching.rs)
- [happy] expands channel_payloads definition into individual channel overrides  (crates/vox-publisher/src/switching.rs)
- [happy] distributes distribution_policy settings (retry_profile) to syndication config  (crates/vox-publisher/src/switching.rs)
- [invariant] unified_news_item_from_manifest_parts() sets syndication.dry_run=true when distribution_policy.dry_run=true regardless of syndication.dry_run metadata  (crates/vox-publisher/src/switching.rs)
- [happy] unified_news_item_from_manifest_parts() parses RFC3339 published_at timestamp correctly  (crates/vox-publisher/src/switching.rs)
- [happy] unified_news_item_from_manifest_parts() expands channels array and channel_payloads into individual syndication fields with distribution_policy properties  (crates/vox-publisher/src/switching.rs)

### `unified_news_item_from_manifest_parts_notes()`  (edge, happy; EXTRACTED)
- [happy] detects and merges legacy scientia_distribution key with modern syndication key  (crates/vox-publisher/src/switching.rs)
- [happy] preserves channel settings from legacy key when merging (rss, twitter.thread)  (crates/vox-publisher/src/switching.rs)
- [edge] generates warning when deprecated root channel_policy is used  (crates/vox-publisher/src/switching.rs)
- [edge] generates warning when reserved crosspost_plan field is present  (crates/vox-publisher/src/switching.rs)
- [edge] generates warning when channel_payloads definition is missing for declared channel  (crates/vox-publisher/src/switching.rs)
- [happy] unified_news_item_from_manifest_parts_notes() merges scientia_distribution legacy key and marks used_legacy_distribution_key=true  (crates/vox-publisher/src/switching.rs)
- [edge] unified_news_item_from_manifest_parts_notes() emits warnings for deprecated channel_policy, reserved crosspost_plan, and missing channel_payloads  (crates/vox-publisher/src/switching.rs)

### `embedded_profiles()`  (happy; EXTRACTED)
- [happy] embedded_profiles() loads profiles containing 'short_insight_thread' for social media syndication  (crates/vox-publisher/src/distribution_compile.rs)
- [happy] embedded_profiles() loads fallback profile for content projection  (crates/vox-publisher/src/distribution_compile.rs)
- [happy] embedded_profiles() loads projection profiles including 'short_insight_thread' and 'fallback'  (crates/vox-publisher/src/distribution_compile.rs)
- [happy] Embedded profiles load with 'short_insight_thread' key present  (crates/vox-publisher/src/distribution_compile.rs)
- [happy] Embedded profiles load with 'fallback' key present  (crates/vox-publisher/src/distribution_compile.rs)
- [happy] embedded_profiles() returns a structure containing 'short_insight_thread' and 'fallback' profile keys  (crates/vox-publisher/src/distribution_compile.rs)

### `evaluate_publish_gate()`  (happy; EXTRACTED)
- [happy] evaluate_publish_gate() returns live_publish_allowed=true when all guard conditions are met  (crates/vox-publisher/src/gate.rs)
- [happy] evaluate_publish_gate() has no blocking reasons when all guards are satisfied  (crates/vox-publisher/src/gate.rs)
- [happy] evaluate_publish_gate() returns live_publish_allowed=true with no blockers when all guards are satisfied  (crates/vox-publisher/src/gate.rs)
- [happy] When all publish guard conditions are met, live_publish_allowed is true  (crates/vox-publisher/src/gate.rs)
- [happy] When all guards met, publish gate has no blocking reasons  (crates/vox-publisher/src/gate.rs)
- [happy] evaluate_publish_gate() returns live_publish_allowed=true and has_blockers()=false when all guard conditions are met  (crates/vox-publisher/src/gate.rs)

### `load_contract_from_str()`  (happy; EXTRACTED)
- [happy] load_contract_from_str() successfully deserializes the default publication-worthiness YAML contract  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] Default worthiness contract loads successfully from YAML string  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] contract loads successfully and enables worthiness decision attachment to preflight report  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [happy] load_contract_from_str() successfully parses the default publication-worthiness.yaml contract without error  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] load_contract_from_str() successfully parses worthiness contract from YAML string.  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [happy] Successfully parses default publication-worthiness YAML into PublicationWorthinessContract  (crates/vox-publisher/src/publication_worthiness.rs)

### `template_source()`  (invariant; EXTRACTED)
- [invariant] template_source(NewsTemplateId::ResearchUpdate) matches docs/news/templates/research_update.md exactly  (crates/vox-publisher/src/templates.rs)
- [invariant] template_source(NewsTemplateId::Release) matches docs/news/templates/release.md exactly  (crates/vox-publisher/src/templates.rs)
- [invariant] template_source(NewsTemplateId::SecurityAdvisory) matches docs/news/templates/security_advisory.md exactly  (crates/vox-publisher/src/templates.rs)
- [invariant] template_source(NewsTemplateId::CommunityUpdate) matches docs/news/templates/community_update.md exactly  (crates/vox-publisher/src/templates.rs)
- [invariant] template_source(NewsTemplateId::DiscordAnnouncement) matches docs/news/templates/discord_announcement.md exactly  (crates/vox-publisher/src/templates.rs)

### `PeerReviewGate.check()`  (error; EXTRACTED)
- [error] check() with single approval returns PeerReviewGateError::InsufficientApprovals  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] check() with one rejection returns PeerReviewGateError::Rejected  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] check() returns PeerReviewGateError::DigestMismatch when review publication_digest differs from expected digest  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] check() returns PeerReviewGateError::InvalidSignature when review signature_hex is tampered  (crates/vox-publisher/src/peer_review_gate.rs)

### `plan_publication_retry_channels()`  (happy; EXTRACTED)
- [happy] excludes channels with Success outcome from retry plan when explicitly listed  (crates/vox-publisher/src/switching.rs)
- [happy] tracks skipped_success_channels when explicit channel list is provided  (crates/vox-publisher/src/switching.rs)
- [happy] includes only failed channels in retry plan when no explicit channel list given  (crates/vox-publisher/src/switching.rs)
- [happy] leaves skipped_success_channels empty in auto-mode retry planning  (crates/vox-publisher/src/switching.rs)

### `validate_contract_invariants()`  (happy, invariant; EXTRACTED)
- [happy] validate_contract_invariants() succeeds on the default publication-worthiness contract  (crates/vox-publisher/src/publication_worthiness.rs)
- [invariant] Default worthiness contract satisfies all invariants  (crates/vox-publisher/src/publication_worthiness.rs)
- [invariant] validate_contract_invariants() passes for the default contract (weights sum to 1.0, thresholds in valid range)  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] Succeeds on default contract without errors  (crates/vox-publisher/src/publication_worthiness.rs)

### `validate_topic_pack_projection_profiles()`  (happy, invariant; EXTRACTED)
- [happy] validate_topic_pack_projection_profiles() successfully validates all embedded topic pack template profile IDs are valid  (crates/vox-publisher/src/distribution_compile.rs)
- [invariant] validate_topic_pack_projection_profiles() successfully validates all topic pack projection profile references  (crates/vox-publisher/src/distribution_compile.rs)
- [invariant] All topic pack template_profile IDs are valid and resolve  (crates/vox-publisher/src/distribution_compile.rs)
- [happy] validate_topic_pack_projection_profiles() succeeds with all template_profile ids valid  (crates/vox-publisher/src/distribution_compile.rs)

### `PeerReviewGate`  (error; EXTRACTED)
- [error] PeerReviewGate.check() rejects with InsufficientApprovals error when fewer than required approvals provided  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] PeerReviewGate.check() rejects with Rejected error when a review contains rejection decision  (crates/vox-publisher/src/peer_review_gate.rs)
- [error] PeerReviewGate.check() rejects with DigestMismatch error when publication digest does not match review digest  (crates/vox-publisher/src/peer_review_gate.rs)

### `hard_metrics_ok()`  (happy; EXTRACTED)
- [happy] evaluate_worthiness() returns hard_metrics_ok=true for publish-ready inputs  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] hard_metrics_ok() returns true when all hard metrics (claim_evidence_coverage, effective_replayability, before_after_pair_integrity, metadata_completeness, ai_disclosure_compliance) meet their thresholds  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] When evaluating publish-ready inputs, hard_metrics_ok returns true  (crates/vox-publisher/src/publication_worthiness.rs)

### `spdx_license_url()`  (happy; EXTRACTED)
- [happy] spdx_license_url() maps MIT SPDX identifier to https://opensource.org/licenses/MIT  (crates/vox-publisher/src/crossref_metadata.rs)
- [happy] spdx_license_url() maps SPDX identifier 'MIT' to https://opensource.org/licenses/MIT  (crates/vox-publisher/src/crossref_metadata.rs)
- [happy] MIT SPDX license maps to https://opensource.org/licenses/MIT  (crates/vox-publisher/src/crossref_metadata.rs)

### `AtlasSubmissionGate::check()`  (error; EXTRACTED)
- [error] returns Err(AtlasGateError::ReplyWindowNotCleared) when reply_window_cleared is false  (crates/vox-publisher/src/atlas/submission.rs)
- [error] returns Err(AtlasGateError::NegativeResultQuotaNotMet) when require_negative_result is true and manifest has no negative findings  (crates/vox-publisher/src/atlas/submission.rs)

### `PreflightProfile::DoubleBlind`  (error; EXTRACTED)
- [error] DoubleBlind profile detects email addresses in body markdown and fails validation.  (crates/vox-publisher/src/publication_preflight/tests.rs)
- [error] DoubleBlind profile detects ORCID URLs in body markdown and fails validation.  (crates/vox-publisher/src/publication_preflight/tests.rs)

### `WorthinessEvaluation::hard_metrics_ok`  (happy; EXTRACTED)
- [happy] Returns true when all hard metric thresholds are met  (crates/vox-publisher/src/publication_worthiness.rs)
- [happy] Remains true when measured replayability meets threshold  (crates/vox-publisher/src/publication_worthiness.rs)

### `failed_channels_from_latest_digest_attempt()`  (error; EXTRACTED)
- [error] returns error when syndication outcome_json contains malformed JSON  (crates/vox-publisher/src/switching.rs)
- [error] failed_channels_from_latest_digest_attempt() propagates JSON parse errors from malformed outcome_json instead of swallowing them  (crates/vox-publisher/src/switching.rs)

### `merge_topic_pack_into_syndication()`  (happy, invariant; EXTRACTED)
- [happy] merge_topic_pack_into_syndication() removes Twitter from social channels while preserving RSS when applied to research_breakthrough pack  (crates/vox-publisher/src/topic_packs.rs)
- [invariant] merge_topic_pack_into_syndication() respects existing higher worthiness floor (>= 0.9) when merging pack policies  (crates/vox-publisher/src/topic_packs.rs)

### `parse_channels_csv()`  (happy; EXTRACTED)
- [happy] normalizes channel names to lowercase and trims whitespace  (crates/vox-publisher/src/switching.rs)
- [happy] parse_channels_csv() normalizes channel names to lowercase and removes whitespace  (crates/vox-publisher/src/switching.rs)

### `record_publication_succeeded()`  (happy; EXTRACTED)
- [happy] Produces JSON output that validates against research-mesh-intake.v1.schema.json  (crates/vox-publisher/src/research_mesh.rs)
- [happy] record_publication_succeeded() writes valid JSON conforming to research-mesh-intake.v1.schema.json  (crates/vox-publisher/src/research_mesh.rs)

### `render_citation_cff()`  (happy; EXTRACTED)
- [happy] render_citation_cff() produces YAML output containing CFF version, title, author name, SPDX license, and abstract text  (crates/vox-publisher/src/citation_cff.rs)
- [happy] render_citation_cff() outputs valid YAML with cff-version 1.2.0, title, author names, and SPDX license  (crates/vox-publisher/src/citation_cff.rs)

### `ArxivAssistAdapter::fetch_status()`  (happy; EXTRACTED)
- [happy] ArxivAssistAdapter::fetch_status() returns status with status field set to pending_operator.  (crates/vox-publisher/src/scholarly/arxiv_api.rs)

### `ArxivAssistAdapter::submit()`  (happy; EXTRACTED)
- [happy] ArxivAssistAdapter::submit() returns receipt with adapter=arxiv_assist, status=staged, and external_submission_id prefixed with arxiv-.  (crates/vox-publisher/src/scholarly/arxiv_api.rs)

### `DiscoveryIntakeTier`  (happy; EXTRACTED)
- [happy] rank_candidate() with passing eval_gate assigns intake_tier=StrongCandidate and rank_score >= 10  (crates/vox-publisher/src/scientia_discovery.rs)

### `PreflightProfile::ArxivAssist`  (error; EXTRACTED)
- [error] ArxivAssist profile requires abstract but does not require scientific_metadata_required; missing abstract causes validation failure.  (crates/vox-publisher/src/publication_preflight/tests.rs)

### `PreflightProfile::MetadataComplete`  (error; EXTRACTED)
- [error] MetadataComplete profile requires scientific metadata block and fails validation when absent.  (crates/vox-publisher/src/publication_preflight/tests.rs)

### `PublishGateInputs evaluation with all guards met`  (happy; EXTRACTED)
- [happy] evaluate_publish_gate returns live_publish_allowed=true when orchestrator_dry_run, item_dry_run are false and publish_armed_config, db_present, dual_approval_met are true  (crates/vox-publisher/src/gate.rs)

### `SyndicationConfig`  (happy; EXTRACTED)
- [happy] SyndicationConfig can be constructed and configured with Twitter social channel  (crates/vox-publisher/src/distribution_compile.rs)

### `WorthinessResult.hard_metrics_ok`  (happy; EXTRACTED)
- [happy] hard_metrics_ok is true when all hard metrics pass the contract floor  (crates/vox-publisher/src/publication_worthiness.rs)

### `apply_topic_pack_from_metadata_json()`  (happy; EXTRACTED)
- [happy] apply_topic_pack_from_metadata_json() applies topic_pack field from metadata and sets channel worthiness floor (e.g., YouTube to 0.8 for benchmark pack)  (crates/vox-publisher/src/topic_packs.rs)

### `consume_pending_intake()`  (happy; EXTRACTED)
- [happy] consume_pending_intake() promotes intake files to ledger, moves files to processed directory, and returns summary with promoted count  (crates/vox-publisher/src/research_mesh.rs)

### `has_blockers()`  (happy; EXTRACTED)
- [happy] has_blockers() returns false when all publish gate guards are met  (crates/vox-publisher/src/gate.rs)

### `intake_gate_allows()`  (happy; EXTRACTED)
- [happy] intake_gate_allows(StrongSignalsOnly, ...) returns true for strong candidate ranks and false for low/default context  (crates/vox-publisher/src/scientia_discovery.rs)

### `load_topic_packs_embedded()`  (happy; EXTRACTED)
- [happy] load_topic_packs_embedded() returns version=1 and loads 'research_breakthrough' pack with non-empty description and min_worthiness_score > 0.8 for github  (crates/vox-publisher/src/topic_packs.rs)

### `operator_status_surface_v1()`  (invariant; EXTRACTED)
- [invariant] operator_status_surface_v1() serializes without leaking secret environment variable values in JSON output.  (crates/vox-publisher/src/publication_preflight/tests.rs)

### `parse_evidence_pack_v1()`  (happy; EXTRACTED)
- [happy] parse_evidence_pack_v1() deserializes valid evidence pack JSON and returns version='v1' with non-empty replay_instructions  (crates/vox-publisher/src/scientia_contracts.rs)

### `parse_route_profile_requirements_v1()`  (happy; EXTRACTED)
- [happy] parse_route_profile_requirements_v1() returns version='v1' and contains 'journal' profile with required_fields including 'identity.title'  (crates/vox-publisher/src/scientia_contracts.rs)

### `template_source(NewsTemplateId::Release)`  (invariant; EXTRACTED)
- [invariant] matches the documentation template at docs/news/templates/release.md  (crates/vox-publisher/src/templates.rs)

### `template_source(NewsTemplateId::ResearchUpdate)`  (invariant; EXTRACTED)
- [invariant] matches the documentation template at docs/news/templates/research_update.md  (crates/vox-publisher/src/templates.rs)

### `template_source(NewsTemplateId::SecurityAdvisory)`  (invariant; EXTRACTED)
- [invariant] matches the documentation template at docs/news/templates/security_advisory.md  (crates/vox-publisher/src/templates.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`ArxivAssistAdapter::fetch_status()`** — only: _ArxivAssistAdapter::fetch_status() returns status with status field set to pending_operator._
- **`ArxivAssistAdapter::submit()`** — only: _ArxivAssistAdapter::submit() returns receipt with adapter=arxiv_assist, status=staged, and external_submission_id prefixed with arxiv-._
- **`DiscoveryIntakeTier`** — only: _rank_candidate() with passing eval_gate assigns intake_tier=StrongCandidate and rank_score >= 10_
- **`PublishGateInputs evaluation with all guards met`** — only: _evaluate_publish_gate returns live_publish_allowed=true when orchestrator_dry_run, item_dry_run are false and publish_armed_config, db_present, dual_approval_met are true_
- **`SyndicationConfig`** — only: _SyndicationConfig can be constructed and configured with Twitter social channel_
- **`WorthinessEvaluation::hard_metrics_ok`** — only: _Returns true when all hard metric thresholds are met_
- **`WorthinessResult.hard_metrics_ok`** — only: _hard_metrics_ok is true when all hard metrics pass the contract floor_
- **`apply_topic_pack_from_metadata_json()`** — only: _apply_topic_pack_from_metadata_json() applies topic_pack field from metadata and sets channel worthiness floor (e.g., YouTube to 0.8 for benchmark pack)_
- **`consume_pending_intake()`** — only: _consume_pending_intake() promotes intake files to ledger, moves files to processed directory, and returns summary with promoted count_
- **`effective_replayability()`** — only: _effective_replayability() returns the declared artifact_replayability value when artifact_replayability_measured is None_
- **`embedded_profiles()`** — only: _embedded_profiles() loads profiles containing 'short_insight_thread' for social media syndication_
- **`evaluate_publish_gate()`** — only: _evaluate_publish_gate() returns live_publish_allowed=true when all guard conditions are met_
- **`hard_metrics_ok()`** — only: _evaluate_worthiness() returns hard_metrics_ok=true for publish-ready inputs_
- **`has_blockers()`** — only: _has_blockers() returns false when all publish gate guards are met_
- **`intake_gate_allows()`** — only: _intake_gate_allows(StrongSignalsOnly, ...) returns true for strong candidate ranks and false for low/default context_
- **`load_contract_from_str()`** — only: _load_contract_from_str() successfully deserializes the default publication-worthiness YAML contract_
- **`load_topic_packs_embedded()`** — only: _load_topic_packs_embedded() returns version=1 and loads 'research_breakthrough' pack with non-empty description and min_worthiness_score > 0.8 for github_
- **`parse_channels_csv()`** — only: _normalizes channel names to lowercase and trims whitespace_
- **`parse_evidence_pack_v1()`** — only: _parse_evidence_pack_v1() deserializes valid evidence pack JSON and returns version='v1' with non-empty replay_instructions_
- **`parse_route_profile_requirements_v1()`** — only: _parse_route_profile_requirements_v1() returns version='v1' and contains 'journal' profile with required_fields including 'identity.title'_
- **`plan_publication_retry_channels()`** — only: _excludes channels with Success outcome from retry plan when explicitly listed_
- **`record_publication_succeeded()`** — only: _Produces JSON output that validates against research-mesh-intake.v1.schema.json_
- **`render_citation_cff()`** — only: _render_citation_cff() produces YAML output containing CFF version, title, author name, SPDX license, and abstract text_
- **`spdx_license_url()`** — only: _spdx_license_url() maps MIT SPDX identifier to https://opensource.org/licenses/MIT_
