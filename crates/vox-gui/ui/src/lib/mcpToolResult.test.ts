import { describe, expect, it } from 'vitest';
import {
  parseGraphifyStatus,
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

  // F-02: invokeMcpTool's Promise<{...}> signature lies about non-nullability
  // - it's a thin passthrough to Tauri invoke() which can resolve null. These
  // three helpers must not throw a raw TypeError when handed a null envelope.
  it('parsePendingApprovals returns [] for a null envelope', () => {
    expect(parsePendingApprovals(null as never)).toEqual([]);
  });

  it('parseGraphifyStatus throws an honest error for a null envelope', () => {
    expect(() => parseGraphifyStatus(null as never)).toThrow(
      'vox_search_status: no response from backend',
    );
  });

  it('parseMcpToolText returns null for a null envelope', () => {
    expect(parseMcpToolText(null as never)).toBeNull();
  });
});
