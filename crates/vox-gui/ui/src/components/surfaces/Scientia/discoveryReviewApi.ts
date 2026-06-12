import { invoke } from '@tauri-apps/api/core';

/**
 * Typed wrappers over the P2/P3 Tauri commands that back the Discovery Review
 * surface. Every invoke arg key is camelCase — the Tauri boundary deserializes
 * these into the snake_case Rust command parameters.
 *
 * NOTE: the decision strings sent to `record_publication_claim_review` MUST be
 * exactly the values the backend validates (`vox-db` `VALID_DECISIONS`):
 * `approved` | `rejected` | `deferred` | `edited`. The buttons map to the first
 * three; sending `approve`/`reject`/`defer` would be rejected by the validator.
 */

/** One claim awaiting human review for a publication (queue row). */
export interface ClaimAwaitingReview {
  claim_id: number;
  text: string;
  is_numeric: boolean;
  verdict: string | null;
  confidence: number | null;
  verifier_model: string | null;
  created_at_ms: number;
}

/** Exact decision strings the backend accepts. */
export type ReviewDecision = 'approved' | 'rejected' | 'deferred' | 'edited';

/** Result of an offline nanopublication build/sign/validate. */
export interface NanopubResult {
  trusty_uri: string;
  published_state: string;
  validated_offline: boolean;
}

/** A single LLM-suggested evidence improvement (Phase 4 backend). */
export interface EvidenceSuggestion {
  kind: string;
  summary: string;
  rationale: string;
}

/** List the claims still awaiting review for a publication. */
export function listReviewQueue(publication_id: string): Promise<ClaimAwaitingReview[]> {
  return invoke<ClaimAwaitingReview[]>('list_publication_review_queue', {
    publicationId: publication_id,
  });
}

/** Record a human review decision for a single claim. */
export function recordDecision(
  publication_id: string,
  claim_id: number,
  decision: ReviewDecision,
  reason?: string,
): Promise<void> {
  return invoke<void>('record_publication_claim_review', {
    publicationId: publication_id,
    claimId: claim_id,
    decision,
    reason: reason ?? null,
  });
}

/** Build + sign + offline-validate a nanopublication for an approved claim. */
export function nanopublish(
  publication_id: string,
  claim_id: number,
  orcid?: string,
): Promise<NanopubResult> {
  return invoke<NanopubResult>('nanopublish_approved_claim', {
    publicationId: publication_id,
    claimId: claim_id,
    orcid: orcid ?? null,
  });
}

/**
 * Ask the LLM (via the model-agnostic facade) for evidence improvements.
 * Backed by the `suggest_evidence_improvements` command added in Phase 4 — until
 * then this rejects, which the caller handles gracefully via pushToast.
 */
export function suggestEvidence(
  publication_id: string,
  claim_id: number,
): Promise<EvidenceSuggestion[]> {
  return invoke<EvidenceSuggestion[]>('suggest_evidence_improvements', {
    publicationId: publication_id,
    claimId: claim_id,
  });
}
