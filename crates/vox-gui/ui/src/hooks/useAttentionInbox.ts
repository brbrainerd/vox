import { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { voxTransport, feedbackList, feedbackResolve, listenFeedbackChanged, hopperList, type FeedbackRow, type HopperTaskDto } from '../transport';
import { parsePendingApprovals, type McpInvokeResult, type PendingApprovalRow } from '../lib/mcpToolResult';
import { ATTENTION_POLL_MS } from '../config/constants';

export interface AttentionInbox {
  approvals: PendingApprovalRow[];
  needsYou: FeedbackRow[];
  withheld: FeedbackRow[];
  blockedTasksCount: number;
  /** Items awaiting a human decision: pending approvals + needs-you feedback. */
  totalCount: number;
  refresh(): Promise<void>;
  resolveApproval(approvalId: string, outcome: 'approved' | 'rejected'): Promise<void>;
  resolveFeedback(feedbackId: string, action: Record<string, unknown>): Promise<void>;
}

export function useAttentionInbox(): AttentionInbox {
  const [approvals, setApprovals] = useState<PendingApprovalRow[]>([]);
  const [needsYou, setNeedsYou] = useState<FeedbackRow[]>([]);
  const [withheld, setWithheld] = useState<FeedbackRow[]>([]);
  const [blockedTasksCount, setBlockedTasksCount] = useState(0);

  const refresh = useCallback(async () => {
    const emptyFeedback = { needsYou: [] as FeedbackRow[], withheld: [] as FeedbackRow[] };
    const [approvalRes, feedback, tasks] = await Promise.all([
      Promise.resolve(voxTransport.invokeMcpTool('vox_pending_approvals', {})).catch(() => null),
      Promise.resolve(feedbackList()).catch(() => emptyFeedback),
      Promise.resolve(hopperList()).catch(() => [] as HopperTaskDto[]),
    ]);
    const safeFeedback = feedback ?? emptyFeedback;
    const safeTasks = tasks ?? [];
    setApprovals(approvalRes ? parsePendingApprovals(approvalRes as McpInvokeResult) : []);
    setNeedsYou(safeFeedback.needsYou ?? []);
    setWithheld(safeFeedback.withheld ?? []);
    const gates = new Set<number>((safeFeedback.needsYou ?? []).flatMap((f) => f.gates ?? []));
    setBlockedTasksCount(safeTasks.filter((t) => gates.has(t.task_id)).length);
  }, []);

  useEffect(() => {
    refresh();
    let unFeedback: (() => void) | null = null;
    let unTasks: (() => void) | null = null;
    Promise.resolve(listenFeedbackChanged(() => { refresh(); })).then((u) => { unFeedback = u ?? null; }).catch(() => {});
    Promise.resolve(listen<void>('vox://tasks-changed', () => { refresh(); })).then((u) => { unTasks = u ?? null; }).catch(() => {});
    const id = setInterval(refresh, ATTENTION_POLL_MS);
    return () => { unFeedback?.(); unTasks?.(); clearInterval(id); };
  }, [refresh]);

  const resolveApproval = useCallback(async (approvalId: string, outcome: 'approved' | 'rejected') => {
    await voxTransport.invokeMcpTool('vox_resolve_approval', { approval_id: approvalId, outcome });
    setApprovals((prev) => prev.filter((a) => a.approval_id !== approvalId));
    await refresh();
  }, [refresh]);

  const resolveFeedback = useCallback(async (feedbackId: string, action: Record<string, unknown>) => {
    await feedbackResolve(feedbackId, action);
    await refresh();
  }, [refresh]);

  return { approvals, needsYou, withheld, blockedTasksCount, totalCount: approvals.length + needsYou.length, refresh, resolveApproval, resolveFeedback };
}
