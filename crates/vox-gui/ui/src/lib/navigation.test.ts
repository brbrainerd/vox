import { describe, expect, it } from 'vitest';
import {
  resolveNavigation,
  parseViewFromLocation,
  breadcrumbsForView,
  TOP_LEVEL_VIEWS,
  DEFAULT_CHILD_BY_PARENT,
  LEGACY_VIEW_ALIASES,
  CHILD_ORDER_BY_PARENT,
  orderedChildren,
  labelForNavKey,
} from './navigation';
import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';

describe('intent-first top-level order', () => {
  it('orders groups: Direct, Review, Agents, Knowledge, Workspace, Commands, Compute, Mercatus, Settings', () => {
    expect([...TOP_LEVEL_VIEWS]).toEqual([
      'chat', 'runs', 'agents', 'knowledge', 'workspace', 'commands', 'compute', 'mercatus', 'settings',
    ]);
  });
  it('retires the Search group', () => {
    expect(TOP_LEVEL_VIEWS).not.toContain('search');
    expect(DEFAULT_CHILD_BY_PARENT.search).toBeUndefined();
  });
  it('labels the runs group Review', () => {
    expect(labelForNavKey('runs')).toBe('Review');
  });
});

describe('Review group (runs parent)', () => {
  it('deep-links approvals to runs parent', () => {
    expect(resolveNavigation('approvals')).toEqual({ parent: 'runs', child: 'approvals' });
  });
  it('wires needs-you into nav under runs', () => {
    expect(resolveNavigation('needs-you')).toEqual({ parent: 'runs', child: 'needs-you' });
  });
  it('promotes runs to a named child of its own group', () => {
    expect(resolveNavigation('runs')).toEqual({ parent: 'runs', child: 'runs' });
  });
  it('keeps approvals as the sidebar landing child for the Review group', () => {
    expect(DEFAULT_CHILD_BY_PARENT.runs).toBe('approvals');
  });
});

describe('group moves', () => {
  it('moves mesh from compute to agents', () => {
    expect(resolveNavigation('mesh').parent).toBe('agents');
  });
  it('moves sub-agents under agents (wired via subagent_tree)', () => {
    expect(resolveNavigation('sub-agents').parent).toBe('agents');
  });
  it('moves gamify from agents to settings', () => {
    expect(resolveNavigation('gamify')).toEqual({ parent: 'settings', child: 'gamify' });
  });
  it('reparents memory under knowledge and makes it the default child', () => {
    expect(resolveNavigation('memory')).toEqual({ parent: 'knowledge', child: 'memory' });
    expect(resolveNavigation('knowledge')).toEqual({ parent: 'knowledge', child: 'memory' });
  });
  it('wires the consolidated Discovery surface (activity) under knowledge', () => {
    expect(resolveNavigation('activity')).toEqual({ parent: 'knowledge', child: 'activity' });
  });
});

describe('legacy alias redirects (deep-links must not break)', () => {
  it('claims and review resolve to scientia', () => {
    expect(resolveNavigation('claims')).toEqual({ parent: 'knowledge', child: 'scientia' });
    expect(resolveNavigation('review')).toEqual({ parent: 'knowledge', child: 'scientia' });
  });
  it('discovery clones resolve to the one Discovery surface', () => {
    for (const legacy of ['discovery-inbox', 'discovery-review', 'archive-panel']) {
      expect(resolveNavigation(legacy)).toEqual({ parent: 'knowledge', child: 'activity' });
    }
  });
  it('matrix folds into chat', () => {
    expect(resolveNavigation('matrix')).toEqual({ parent: 'chat', child: 'chat' });
  });
  it('search resolves to memory', () => {
    expect(resolveNavigation('search')).toEqual({ parent: 'knowledge', child: 'memory' });
  });
  it('exposes the alias map for callers that seed presets', () => {
    expect(LEGACY_VIEW_ALIASES['discovery-inbox']).toBe('activity');
  });
});

describe('child ordering', () => {
  it('orders Review children approvals-first', () => {
    expect(CHILD_ORDER_BY_PARENT.runs).toEqual(['approvals', 'needs-you', 'runs', 'policies']);
  });
  it('orders Workspace children console-first', () => {
    expect(orderedChildren('workspace', ['browser', 'console', 'harness', 'repository']))
      .toEqual(['console', 'repository', 'browser', 'harness']);
  });
  it('orders Knowledge children memory-first', () => {
    expect(orderedChildren('knowledge', ['activity', 'memory', 'publications', 'research', 'scientia', 'vox-search']))
      .toEqual(['memory', 'scientia', 'research', 'activity', 'publications', 'vox-search']);
  });
  it('passes unknown parents through untouched', () => {
    expect(orderedChildren('mercatus', ['a', 'b'])).toEqual(['a', 'b']);
  });
});

describe('needs-you attention inbox nav wiring', () => {
  // resolveNavigation('needs-you') -> { parent: 'runs', child: 'needs-you' } is already
  // covered by the 'wires needs-you into nav under runs' test above; not duplicated here.
  it('labels needs-you for breadcrumbs', () => {
    expect(labelForNavKey('needs-you')).toBe('Needs You');
  });
  it('registry parents needs-you under runs so ParentSurface shows the tab', () => {
    const entry = SURFACE_REGISTRY.find(e => e.viewKey === 'needs-you');
    expect(entry?.parentSurface).toBe('runs');
  });
});

describe('unchanged plumbing', () => {
  it('parseViewFromLocation reads hash and query', () => {
    expect(parseViewFromLocation({ hash: '#view=console', search: '' })).toBe('console');
    expect(parseViewFromLocation({ hash: '', search: '?view=memory' })).toBe('memory');
    expect(parseViewFromLocation({ hash: '', search: '' })).toBeNull();
  });
  it('breadcrumbsForView includes parent and child', () => {
    expect(breadcrumbsForView('console').map(c => c.key)).toEqual(['workspace', 'console']);
  });
});
