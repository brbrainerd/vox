/**
 * Resolve a view key to its top-level nav parent and optional child tab.
 */
export const PARENT_CHILD_MAP: Record<string, { parent: string; child?: string }> = {
  dashboard: { parent: 'agents', child: 'dashboard' },
  flow: { parent: 'agents', child: 'flow' },
  matrix: { parent: 'agents', child: 'matrix' },
  approvals: { parent: 'runs', child: 'approvals' },
  policies: { parent: 'runs', child: 'policies' },
  repository: { parent: 'workspace', child: 'repository' },
  browser: { parent: 'workspace', child: 'browser' },
  harness: { parent: 'workspace', child: 'harness' },
  console: { parent: 'workspace', child: 'console' },
  catalog: { parent: 'commands', child: 'catalog' },
  skills: { parent: 'commands', child: 'skills' },
  memory: { parent: 'search', child: 'memory' },
  research: { parent: 'knowledge', child: 'research' },
  scientia: { parent: 'knowledge', child: 'scientia' },
  'discovery-review': { parent: 'knowledge', child: 'discovery-review' },
  claims: { parent: 'knowledge', child: 'claims' },
  publications: { parent: 'knowledge', child: 'publications' },
  models: { parent: 'compute', child: 'models' },
  mens: { parent: 'compute', child: 'mens' },
  populi: { parent: 'compute', child: 'populi' },
  oratio: { parent: 'compute', child: 'oratio' },
  mesh: { parent: 'compute', child: 'mesh' },
  coverage: { parent: 'settings', child: 'coverage' },
  gamify: { parent: 'settings', child: 'gamify' },
};

export const TOP_LEVEL_VIEWS = [
  'chat',
  'agents',
  'runs',
  'workspace',
  'commands',
  'search',
  'knowledge',
  'compute',
  'settings',
] as const;

export type TopLevelView = typeof TOP_LEVEL_VIEWS[number];

export function resolveNavigation(viewKey: string): { parent: string; child: string } {
  const mapped = PARENT_CHILD_MAP[viewKey];
  if (mapped) {
    return { parent: mapped.parent, child: mapped.child ?? viewKey };
  }
  if (TOP_LEVEL_VIEWS.includes(viewKey as TopLevelView)) {
    const children = Object.entries(PARENT_CHILD_MAP).filter(([, v]) => v.parent === viewKey);
    const defaultChild = children[0]?.[0] ?? viewKey;
    return {
      parent: viewKey,
      child: viewKey === 'runs' ? 'runs' : viewKey === 'settings' ? 'settings' : defaultChild,
    };
  }
  return { parent: viewKey, child: viewKey };
}
