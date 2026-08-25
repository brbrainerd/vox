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

  // The parse layer rebuilds the status object field by field, so a key it
  // forgets to forward is invisible to every panel test that mocks the hook.
  // That is exactly how the TTL editor shipped dead: the backend emitted
  // ttl_days, the parse dropped it, and the panel's presence guard was always
  // false. This case fails if any TTL key stops being forwarded.
  it('forwards the TTL keys from the vox_search_status envelope', () => {
    const status = parseGraphifyStatus({
      tool: 'vox_search_status',
      is_error: false,
      result: {
        success: true,
        data: {
          default_corpus_id: 'vox',
          corpora: [],
          ttl_days: 14,
          ttl_days_contract: 30,
          ttl_days_env_forced: true,
          ttl_contract_path: 'contracts/retrieval/vox-graph-corpora.v1.yaml',
        },
      },
    });
    expect(status.ttl_days).toBe(14);
    expect(status.ttl_days_contract).toBe(30);
    expect(status.ttl_days_env_forced).toBe(true);
    expect(status.ttl_contract_path).toBe('contracts/retrieval/vox-graph-corpora.v1.yaml');
  });

  it('leaves ttl_days undefined when the backend omits it', () => {
    const status = parseGraphifyStatus({
      tool: 'vox_search_status',
      is_error: false,
      result: { success: true, data: { default_corpus_id: 'vox', corpora: [] } },
    });
    expect(status.ttl_days).toBeUndefined();
    expect(status.default_corpus_id).toBe('vox');
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
