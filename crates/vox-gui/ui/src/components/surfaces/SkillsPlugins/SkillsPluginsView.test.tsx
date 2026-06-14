// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const SKILLS = [
  { id: 'sk1', name: 'Skill One', version: '1.0', category: 'x', description: 'desc', tools: [], source: 'local', permissions: [], tags: [] },
];

const invokeMock = vi.fn((_cmd: string, args: any) => {
  const tool = args?.tool;
  if (tool === 'vox_skill_list') return Promise.resolve({ tool, is_error: false, result: { data: SKILLS } });
  if (tool === 'vox_plugin_list') return Promise.resolve({ tool, is_error: false, result: { data: [] } });
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
});
