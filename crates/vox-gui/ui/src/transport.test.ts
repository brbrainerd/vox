import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { voxTransport } from './transport';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe('VoxTransport new methods', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('logFrontend calls log_frontend with level + message', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await voxTransport.logFrontend('warn', 'test warning');
    expect(mockInvoke).toHaveBeenCalledWith('log_frontend', { level: 'warn', message: 'test warning' });
  });

  it('getGuiPreference returns null when backend returns null', async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await voxTransport.getGuiPreference('dock-layout');
    expect(mockInvoke).toHaveBeenCalledWith('get_gui_preference', { key: 'dock-layout' });
    expect(result).toBeNull();
  });

  it('getGuiPreference returns string value', async () => {
    mockInvoke.mockResolvedValue('{"collapsed":false}');
    const result = await voxTransport.getGuiPreference('dock-layout');
    expect(result).toBe('{"collapsed":false}');
  });

  it('setGuiPreference calls set_gui_preference', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await voxTransport.setGuiPreference('dock-layout', '{"collapsed":true}');
    expect(mockInvoke).toHaveBeenCalledWith('set_gui_preference', { key: 'dock-layout', value: '{"collapsed":true}' });
  });

  it('invokeMcpTool forwards tool name and args', async () => {
    mockInvoke.mockResolvedValue({ is_error: false, result: { ok: true } });
    const result = await voxTransport.invokeMcpTool('vox_skill_list', {});
    expect(mockInvoke).toHaveBeenCalledWith('invoke_mcp_tool', { tool: 'vox_skill_list', args: {} });
    expect(result).toEqual({ is_error: false, result: { ok: true } });
  });

  it('openLocator forwards the locator object and returns the outcome', async () => {
    mockInvoke.mockResolvedValue({ action: 'opened' });
    const locator = { kind: 'file' as const, value: 'src/main.rs' };
    const outcome = await voxTransport.openLocator(locator);
    expect(mockInvoke).toHaveBeenCalledWith('open_locator', { locator });
    expect(outcome).toEqual({ action: 'opened' });
  });

  it('openLocator passes a web locator through unchanged', async () => {
    mockInvoke.mockResolvedValue({ action: 'spawned' });
    const locator = { kind: 'web' as const, value: 'https://example.test' };
    await voxTransport.openLocator(locator);
    expect(mockInvoke).toHaveBeenCalledWith('open_locator', { locator });
  });

  it('voxDocsIndex calls vox_docs_index', async () => {
    mockInvoke.mockResolvedValue([]);
    await voxTransport.voxDocsIndex();
    expect(mockInvoke).toHaveBeenCalledWith('vox_docs_index');
  });

  it('voxSearchQuery calls vox_search_query with scope', async () => {
    mockInvoke.mockResolvedValue({ hits: [], total: 0 });
    await voxTransport.voxSearchQuery('foo', 10, ['repo']);
    expect(mockInvoke).toHaveBeenCalledWith('vox_search_query', {
      query: 'foo',
      limit: 10,
      scope: ['repo'],
    });
  });

  it('getOrchestratorStatusBin calls get_orchestrator_status_bin', async () => {
    const bytes = new Uint8Array([0x80]);
    mockInvoke.mockResolvedValue(bytes);
    const result = await voxTransport.getOrchestratorStatusBin();
    expect(mockInvoke).toHaveBeenCalledWith('get_orchestrator_status_bin');
    expect(result).toBe(bytes);
  });

  it('listPolicies calls policy_list and maps id to name', async () => {
    mockInvoke.mockResolvedValue([
      { id: 'fmt.rust', title: 'Rust formatting', domain: 'format', group: 'Formatting' },
      { id: 'lint.clippy', title: 'Clippy', domain: 'lint', group: 'Lint' },
    ]);
    const result = await voxTransport.listPolicies();
    expect(mockInvoke).toHaveBeenCalledWith('policy_list', { domain: null, group: null });
    expect(result).toEqual([
      { name: 'fmt.rust' },
      { name: 'lint.clippy' },
    ]);
  });

  it('listPolicies returns empty array when backend returns non-array', async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await voxTransport.listPolicies();
    expect(result).toEqual([]);
  });

  it('getIdentitySummary calls get_identity_summary', async () => {
    mockInvoke.mockResolvedValue({ display_name: 'Alice', os_user: 'Alice' });
    const result = await voxTransport.getIdentitySummary();
    expect(mockInvoke).toHaveBeenCalledWith('get_identity_summary');
    expect(result).toEqual({ display_name: 'Alice', os_user: 'Alice' });
  });

  it('getLlmSpend calls get_llm_spend with empty session scope', async () => {
    mockInvoke.mockResolvedValue({
      sessionUsd: 0.1,
      dayUsd: 0.5,
      totalUsd: 3.25,
      dailyBudgetUsd: 10,
      perSessionBudgetUsd: 2,
    });
    const result = await voxTransport.getLlmSpend();
    expect(mockInvoke).toHaveBeenCalledWith('get_llm_spend', {});
    expect(result.totalUsd).toBe(3.25);
  });

  it('getGamifySettings calls get_gamify_settings', async () => {
    mockInvoke.mockResolvedValue({ enabled: false, mode: 'serious' });
    const result = await voxTransport.getGamifySettings();
    expect(mockInvoke).toHaveBeenCalledWith('get_gamify_settings');
    expect(result).toEqual({ enabled: false, mode: 'serious' });
  });

  it('recordGuiEvent calls record_gui_event with hook + metadata', async () => {
    mockInvoke.mockResolvedValue({
      xpGranted: 5,
      lumensGranted: 0,
      achievementTitle: 'XP',
    });
    const result = await voxTransport.recordGuiEvent('chat_message_sent', { session_id: 's1' });
    expect(mockInvoke).toHaveBeenCalledWith('record_gui_event', {
      eventType: 'chat_message_sent',
      metadata: { session_id: 's1' },
    });
    expect(result).toEqual({
      xpGranted: 5,
      lumensGranted: 0,
      achievementTitle: 'XP',
    });
  });

  it('recordGuiEvent passes null metadata when omitted', async () => {
    mockInvoke.mockResolvedValue({
      xpGranted: 0,
      lumensGranted: 0,
      achievementTitle: null,
    });
    await voxTransport.recordGuiEvent('palette_navigation');
    expect(mockInvoke).toHaveBeenCalledWith('record_gui_event', {
      eventType: 'palette_navigation',
      metadata: null,
    });
  });

  it('listOrchestratorTasks calls list_orchestrator_tasks', async () => {
    mockInvoke.mockResolvedValue([
      {
        id: 1,
        description: 'Ship feature',
        priority: 'normal',
        lifecycle: 'queued',
        agent_id: null,
        session_id: 'sess-1',
        estimated_complexity: 2,
        depends_on: [],
        write_files: [],
        remote_node: null,
      },
    ]);
    const result = await voxTransport.listOrchestratorTasks();
    expect(mockInvoke).toHaveBeenCalledWith('list_orchestrator_tasks');
    expect(result).toHaveLength(1);
    expect(result[0]?.description).toBe('Ship feature');
  });
});
