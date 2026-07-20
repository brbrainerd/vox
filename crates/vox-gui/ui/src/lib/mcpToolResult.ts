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

export function parsePendingApprovals(
  invokeResult: McpInvokeResult | null | undefined,
): PendingApprovalRow[] {
  if (!invokeResult) return [];
  const data = unwrapMcpEnvelope(invokeResult.result) as { approvals?: PendingApprovalRow[] } | null;
  const list = data?.approvals;
  return Array.isArray(list) ? list : [];
}

export interface GraphifyStatusPayload {
  default_corpus_id: string;
  corpora: unknown[];
}

/** Parse the `vox_search_status` envelope into the panel's status shape. */
export function parseGraphifyStatus(
  invokeResult: McpInvokeResult | null | undefined,
): GraphifyStatusPayload {
  if (!invokeResult) {
    throw new Error('vox_search_status: no response from backend');
  }
  if (invokeResult.is_error) {
    throw new Error('vox_search_status reported an error');
  }
  const data = unwrapMcpEnvelope(invokeResult.result) as Partial<GraphifyStatusPayload> | null;
  return {
    default_corpus_id: data?.default_corpus_id ?? '',
    corpora: Array.isArray(data?.corpora) ? data.corpora : [],
  };
}

/** Extract string payload from `vox_git_diff` and similar text tools. */
export function parseMcpToolText(invokeResult: McpInvokeResult | null | undefined): string | null {
  if (!invokeResult) return null;
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
