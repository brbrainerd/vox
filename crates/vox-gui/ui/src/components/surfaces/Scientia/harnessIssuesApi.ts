import { invoke } from '@tauri-apps/api/core';

/** One row of `scientia_harness_issues`. */
export interface HarnessIssueRow {
  id: number;
  source: 'chat_session' | 'corpus_scan';
  session_key: string | null;
  target_path: string | null;
  detected_at_ms: number;
  category: string;
  severity: 'low' | 'medium' | 'high';
  summary: string;
  evidence_json: string;
  status: 'pending' | 'confirmed' | 'dismissed';
}

/** One row of `scientia_harness_fix_proposals`. */
export interface HarnessFixProposalRow {
  id: number;
  issue_id: number;
  target_path: string;
  proposed_content: string;
  proposed_diff: string;
  status: 'pending_approval' | 'applied' | 'rejected';
  proposed_at_ms: number;
  resolved_at_ms: number | null;
}

export function listHarnessIssues(status?: string, source?: string): Promise<HarnessIssueRow[]> {
  return invoke<HarnessIssueRow[]>('list_harness_issues', {
    status: status ?? null,
    source: source ?? null,
  });
}

export function listHarnessIssuesForSession(sessionKey: string): Promise<HarnessIssueRow[]> {
  return invoke<HarnessIssueRow[]>('list_harness_issues_for_session', { sessionKey });
}

export function recordHarnessIssueDecision(
  issueId: number,
  decision: 'confirmed' | 'dismissed',
  reason?: string,
): Promise<void> {
  return invoke<void>('record_harness_issue_decision', {
    issueId,
    decision,
    reason: reason ?? null,
  });
}

export function scanTrainingCorpus(): Promise<number> {
  return invoke<number>('scan_training_corpus');
}

export function proposeHarnessIssueFix(issueId: number, targetPath: string): Promise<number> {
  return invoke<number>('propose_harness_issue_fix', { issueId, targetPath });
}

export function listHarnessFixProposals(status?: string): Promise<HarnessFixProposalRow[]> {
  return invoke<HarnessFixProposalRow[]>('list_harness_fix_proposals', { status: status ?? null });
}

export function resolveHarnessFixProposal(proposalId: number, approve: boolean): Promise<void> {
  return invoke<void>('resolve_harness_fix_proposal', { proposalId, approve });
}
