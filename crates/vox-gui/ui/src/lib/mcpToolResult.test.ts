import { describe, expect, it } from 'vitest';
import {
  parseMcpToolText,
  parsePendingApprovals,
  unwrapMcpEnvelope,
} from './mcpToolResult';

describe('mcpToolResult', () => {
  it('unwraps ToolResult data', () => {
    expect(unwrapMcpEnvelope({ success: true, data: { approvals: [] } })).toEqual({
      approvals: [],
    });
  });

  it('parses pending approvals from invoke_mcp_tool shape', () => {
    const rows = parsePendingApprovals({
      tool: 'vox_pending_approvals',
      is_error: false,
      result: {
        success: true,
        data: {
          approvals: [
            {
              approval_id: 'AP-000001',
              tool: 'vox_run_shell',
              summary: 'rm -rf build',
              requested_at_ms: 1000,
            },
          ],
        },
      },
    });
    expect(rows).toHaveLength(1);
    expect(rows[0].approval_id).toBe('AP-000001');
  });

  it('extracts diff text from git diff tool envelope', () => {
    const text = parseMcpToolText({
      tool: 'vox_git_diff',
      is_error: false,
      result: { success: true, data: 'diff --git a/foo b/foo\n' },
    });
    expect(text).toContain('diff --git');
  });
});
