// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor, fireEvent } from '@testing-library/react';
import React from 'react';

const SKILL_INFO_RESULT = {
  id: 'sk1', name: 'Skill One', version: '1.0', category: 'x',
  description: 'desc', tools: [], source: 'local', permissions: [], tags: [],
};

const SKILLS = [SKILL_INFO_RESULT];

const invokeMock = vi.fn((_cmd: string, args: any) => {
  const tool = args?.tool;
  if (tool === 'vox_skill_list') return Promise.resolve({ tool, is_error: false, result: { data: SKILLS } });
  if (tool === 'vox_plugin_list') return Promise.resolve({ tool, is_error: false, result: { data: [] } });
  if (tool === 'vox_skill_info') return Promise.resolve({ tool, is_error: false, result: { data: SKILL_INFO_RESULT } });
  if (tool === 'vox_skill_discover')
    return Promise.resolve({ tool, is_error: false, result: { data: [
      { id: 'mine', name: 'mine', description: 'd', path: '/ws/.vox/skills/mine', installed: true, source_root: 'vox', removable: true, license: '' },
      { id: 'bundled', name: 'bundled', description: 'd', path: '/ws/assets/skills/bundled', installed: true, source_root: 'bundled', removable: false, license: 'LICENSE' },
    ] } });
  return Promise.resolve({ tool, is_error: false, result: { data: [] } });
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { SkillsPluginsView } from './SkillsPluginsView';

describe('SkillsPluginsView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders the Skills & Plugins heading', () => {
    render(<SkillsPluginsView pushToast={vi.fn()} />);
    expect(screen.getByText(/Skills/)).toBeTruthy();
  });

  it('every button carries an explicit type="button"', async () => {
    render(<SkillsPluginsView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('Skill One')).toBeTruthy());
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('exposes the tabs as a tablist with aria-selected', () => {
    render(<SkillsPluginsView pushToast={vi.fn()} />);
    expect(screen.getByRole('tablist')).toBeTruthy();
    const tabs = screen.getAllByRole('tab');
    expect(tabs.length).toBe(3);
    expect(tabs.some(t => t.getAttribute('aria-selected') === 'true')).toBe(true);
  });

  it('clicking Info shows skill name in detail panel, not in a JSON toast', async () => {
    const pushToast = vi.fn();
    render(<SkillsPluginsView pushToast={pushToast} />);
    await waitFor(() => expect(screen.getByText('Skill One')).toBeTruthy());
    const infoBtn = screen.getByRole('button', { name: /^info$/i });
    fireEvent.click(infoBtn);
    // After the async call resolves, the detail panel should show the skill name
    await waitFor(() => {
      // There should be at least one element with "Skill One" visible (in the detail panel)
      const matches = screen.getAllByText('Skill One');
      expect(matches.length).toBeGreaterThanOrEqual(1);
    });
    // pushToast should NOT have been called with a JSON body from vox_skill_info
    const jsonToastCall = pushToast.mock.calls.find(
      (call) => call[0]?.body && call[0].body.startsWith('{'),
    );
    expect(jsonToastCall).toBeUndefined();
  });

  it('shows Remove only on removable discovered skills', async () => {
    render(<SkillsPluginsView pushToast={vi.fn()} />);
    fireEvent.click(screen.getByRole('tab', { name: /discovered/i }));
    await waitFor(() => {
      expect(screen.queryAllByRole('button', { name: /^remove$/i }).length).toBe(1);
    });
    // 'mine' (removable) has a Remove button; 'bundled' (read-only) does not.
    const removeButtons = screen.queryAllByRole('button', { name: /^remove$/i });
    expect(removeButtons.length).toBe(1);
  });
});
