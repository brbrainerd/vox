import { invoke } from '@tauri-apps/api/core';

/**
 * Typed wrappers over the Task-19 Archive-panel Tauri commands. Every invoke
 * arg key is camelCase — the Tauri boundary deserializes these into the
 * snake_case Rust command parameters.
 *
 * The three commands surface the Track B archive pipeline: a publication's
 * metadata-completeness report, a deterministic (provenance-carrying) autofill,
 * and its deposit status (Zenodo DOI / Software Heritage SWHID). All reuse the
 * SAME backend SSOT the CLI uses, so GUI and CLI agree.
 */

/** One provenance entry: which field, where its value came from, optional note. */
export interface FieldProvenance {
  field: string;
  origin: string;
  notes: string | null;
}

/** Metadata-completeness report for one publication's manifest. */
export interface CompletionReport {
  completeness_0_100: number;
  required_missing: string[];
  inferred_ok: string[];
  human_only_pending: string[];
  field_provenance: FieldProvenance[];
}

/** One proposed (or applied) field fill, with provenance. `value` is a JSON string. */
export interface PlannedFill {
  field: string;
  value: string;
  origin: string;
  notes: string | null;
}

/** Result of computing (and optionally applying) the deterministic autofill plan. */
export interface AutofillResult {
  fills: PlannedFill[];
  human_only_remaining: string[];
  completeness_before: number;
  /** Equals `completeness_before` when apply was false. */
  completeness_after: number;
}

/** Deposit status surfaced from whatever is actually persisted (honest nulls). */
export interface ArchiveStatus {
  swhid: string | null;
  swh_task_status: string | null;
  zenodo_doi: string | null;
  zenodo_state: string | null;
}

/** Metadata-completeness report for one publication (read-only). */
export function getCompletionReport(publicationId: string): Promise<CompletionReport> {
  return invoke<CompletionReport>('get_completion_report', { publicationId });
}

/**
 * Compute the deterministic autofill plan; when `apply` is true, persist it via
 * the same SSOT path the CLI `publication-autofill --apply` uses and return the
 * raised after-completeness.
 */
export function runAutofill(publicationId: string, apply: boolean): Promise<AutofillResult> {
  return invoke<AutofillResult>('run_autofill', { publicationId, apply });
}

/** Deposit status (Zenodo DOI/state, SWHID/task) for one publication (read-only). */
export function getArchiveStatus(publicationId: string): Promise<ArchiveStatus> {
  return invoke<ArchiveStatus>('get_archive_status', { publicationId });
}
