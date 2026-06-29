import { useCallback, useEffect, useRef, useState } from 'react';
import { voxTransport } from '../transport';
import { parsePendingApprovals, type PendingApprovalRow, type McpInvokeResult } from '../lib/mcpToolResult';
import { APPROVALS_POLL_MS } from '../config/constants';

export interface UseAgentApprovals {
  /** Total number of pending approvals in the real `vox_pending_approvals` feed. */
  count: number;
  approvalFor(agentKey: string): PendingApprovalRow | null;
  resolve(approvalId: string, outcome: 'approved' | 'rejected'): Promise<void>;
}

export function useAgentApprovals(agentKeys: string[]): UseAgentApprovals {
  const [rows, setRows] = useState<PendingApprovalRow[]>([]);
  const agentKeysRef = useRef(agentKeys);
  agentKeysRef.current = agentKeys;

  const refresh = useCallback(async () => {
    try {
      const res = await voxTransport.invokeMcpTool('vox_pending_approvals', {});
      // invokeMcpTool returns { is_error?, result? }; parsePendingApprovals only reads .result. The missing 'tool' field is intentionally unused.
      const parsed = parsePendingApprovals(res as McpInvokeResult);
      setRows(parsed);
    } catch {
      // silently ignore poll errors
    }
  }, []);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, APPROVALS_POLL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  const approvalFor = useCallback(
    (agentKey: string): PendingApprovalRow | null => {
      const safe = agentKey.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const re = new RegExp(`\\b${safe}\\b`, 'i');
      return rows.find((r) => re.test(`${r.summary} ${r.tool}`)) ?? null;
    },
    [rows],
  );

  const resolve = useCallback(
    async (approvalId: string, outcome: 'approved' | 'rejected'): Promise<void> => {
      await voxTransport.invokeMcpTool('vox_resolve_approval', { approval_id: approvalId, outcome });
      setRows((prev) => prev.filter((r) => r.approval_id !== approvalId));
      await refresh();
    },
    [refresh],
  );

  return { count: rows.length, approvalFor, resolve };
}
