/**
 * Parse envelopes returned by `invoke_mcp_tool` (B5).
 * Daemon tools wrap payloads as `{ success, data, error?, remediation? }`.
 */

export interface McpInvokeResult {
  tool: string;
  is_error: boolean;
  result: unknown;
}

export interface PendingApprovalRow {
  approval_id: string;
  tool: string;
  summary: string;
  requested_at_ms: number;
}

/** Unwrap a ToolResult `.data` field when present. */
export function unwrapMcpEnvelope(result: unknown): unknown {
  if (result && typeof result === 'object' && 'data' in result) {
    return (result as { data: unknown }).data;
  }
  return result;
}

export function parsePendingApprovals(invokeResult: McpInvokeResult): PendingApprovalRow[] {
  const data = unwrapMcpEnvelope(invokeResult.result) as { approvals?: PendingApprovalRow[] } | null;
  const list = data?.approvals;
  return Array.isArray(list) ? list : [];
}

/** Extract string payload from `vox_git_diff` and similar text tools. */
export function parseMcpToolText(invokeResult: McpInvokeResult): string | null {
  const inner = invokeResult.result;
  if (typeof inner === 'string') return inner;
  if (inner && typeof inner === 'object') {
    const envelope = inner as Record<string, unknown>;
    if (envelope.success === true) {
      const data = envelope.data;
      if (typeof data === 'string') return data;
    }
    if (typeof envelope.error === 'string') return null;
  }
  return null;
}
