import { invoke } from '@tauri-apps/api/core';

/**
 * Typed wrapper over the `get_novelty_assessment` Tauri command (Task 17).
 * The invoke arg key is camelCase (`publicationId`) — the Tauri boundary
 * deserializes it into the snake_case Rust parameter `publication_id`, matching
 * the convention in `discoveryReviewApi.ts`.
 */

/** One prior-art hit shown in the evidence panel. */
export interface PriorArtHit {
  work_uri: string;
  title: string;
  year: number | null;
  cited_by_count: number | null;
  semantic_score: number | null;
}

/** One side (supporting / contradicting) of an evidence conflict. */
export interface ConflictHit {
  work_uri: string;
  excerpt: string | null;
}

/** A supporting-vs-contradicting conflict among high-similarity hits. */
export interface Conflict {
  claim_text: string;
  conflict_score: number;
  supporting: ConflictHit[];
  contradicting: ConflictHit[];
}

/** Explainable signal breakdown. */
export interface NoveltySignals {
  max_semantic: number | null;
  max_lexical: number | null;
  near_hit_count: number;
  top_hit_citations: number | null;
  sources_succeeded: number;
}

/** The verdict kinds the backend emits. */
export type NoveltyVerdictKind =
  | 'insufficient_evidence'
  | 'novel'
  | 'possibly_novel'
  | 'not_novel';

/** Full novelty assessment for one publication. */
export interface NoveltyAssessment {
  verdict_kind: NoveltyVerdictKind;
  closest_hit_uri: string | null;
  closest_score: number | null;
  excluded_future_hits: number;
  conflicts: Conflict[];
  signals: NoveltySignals;
  prior_art: PriorArtHit[];
}

/** Assess novelty for one publication from its stored evidence bundle. */
export function getNoveltyAssessment(publicationId: string): Promise<NoveltyAssessment> {
  return invoke<NoveltyAssessment>('get_novelty_assessment', { publicationId });
}
