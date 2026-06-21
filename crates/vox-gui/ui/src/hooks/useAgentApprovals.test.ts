// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';

const invokeMcpTool = vi.fn();
vi.mock('../transport', () => ({ voxTransport: { invokeMcpTool: (...a: unknown[]) => invokeMcpTool(...a) } }));

// Real parsePendingApprovals and unwrapMcpEnvelope run — no mock of mcpToolResult.

import { useAgentApprovals } from './useAgentApprovals';

beforeEach(() => { invokeMcpTool.mockReset(); });

describe('useAgentApprovals', () => {
  it('maps a pending approval to the agent named (word boundary) in its summary', async () => {
    // invokeMcpTool resolves with the real McpInvokeResult shape:
    // parsePendingApprovals reads .result, unwrapMcpEnvelope checks for .data wrapper,
    // then reads .approvals from the unwrapped value.
    invokeMcpTool.mockResolvedValue({
      is_error: false,
      result: { approvals: [{ approval_id: 'ap1', tool: 'shell', summary: 'Atlas wants to run rm', requested_at_ms: 0 }] },
    });
    const { result } = renderHook(() => useAgentApprovals(['Atlas', 'Surveyor']));
    await waitFor(() => expect(result.current.approvalFor('Atlas')).not.toBeNull());
    expect(result.current.approvalFor('Atlas')!.approval_id).toBe('ap1');
    expect(result.current.approvalFor('Surveyor')).toBeNull();
  });

  it('does not match a codename that only appears as a substring', async () => {
    invokeMcpTool.mockResolvedValue({
      is_error: false,
      result: { approvals: [{ approval_id: 'ap2', tool: 't', summary: 'edit atlasian config', requested_at_ms: 0 }] },
    });
    const { result } = renderHook(() => useAgentApprovals(['Atlas']));
    await waitFor(() => expect(invokeMcpTool).toHaveBeenCalled());
    expect(result.current.approvalFor('Atlas')).toBeNull();
  });
});
