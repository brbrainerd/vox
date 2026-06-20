import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const SRC_ROOT = join(import.meta.dirname, '..');

/** Infrastructure files that must route IPC through VoxTransport (Phase 0B). */
const MUST_USE_TRANSPORT = [
  'lib/consoleBridge.ts',
  'hooks/usePersistedDbState.ts',
  'components/layout/DockShell.tsx',
  'components/layout/CommandPalette.tsx',
  'main.tsx',
];

function collectTsFiles(dir: string, acc: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      if (entry === 'guards') continue;
      collectTsFiles(full, acc);
    } else if (/\.(ts|tsx)$/.test(entry) && !entry.endsWith('.test.ts') && !entry.endsWith('.test.tsx')) {
      acc.push(full);
    }
  }
  return acc;
}

describe('IPC boundaries (Phase 0B)', () => {
  it('infrastructure files do not import @tauri-apps/api/core directly', () => {
    const violations: string[] = [];
    for (const rel of MUST_USE_TRANSPORT) {
      const full = join(SRC_ROOT, rel);
      const text = readFileSync(full, 'utf8');
      if (text.includes("from '@tauri-apps/api/core'")) {
        violations.push(rel);
      }
    }
    expect(violations).toEqual([]);
  });

  it('only transport.ts imports invoke in the IPC hub layer (tracked allowlist shrinks per wave)', () => {
    const ALLOW_DIRECT_INVOKE = new Set([
      'transport.ts',
      'App.tsx',
      'components/layout/Sidebar.tsx',
      'components/surfaces/Approvals/ApprovalsView.tsx',
      'components/surfaces/Browser/BrowserView.tsx',
      'components/surfaces/Chat/ChatSurface.tsx',
      'components/surfaces/CommandCardsView.tsx',
      'components/surfaces/Gamify/GamifyView.tsx',
      'components/surfaces/Loquela/InlineApprovals.tsx',
      'components/surfaces/Loquela/Loquela.tsx',
      'components/surfaces/Loquela/oratioVoiceInput.ts',
      'components/surfaces/Matrix/Matrix.tsx',
      'components/surfaces/Memory/MemoryView.tsx',
      'components/surfaces/Mesh/MeshView.tsx',
      'components/surfaces/Models/ModelsView.tsx',
      'components/surfaces/Policies/PoliciesView.tsx',
      'components/surfaces/Publications/PublicationsView.tsx',
      'components/surfaces/Repository/RepositoryView.tsx',
      'components/surfaces/Research/ResearchView.tsx',
      'components/surfaces/Research/researchActions.ts',
      'components/surfaces/Runs/RunsView.tsx',
      'components/surfaces/Scientia/ClaimsView.tsx',
      'components/surfaces/Scientia/DiscoveryReviewView.tsx',
      'components/surfaces/Scientia/ScientiaDashboard.tsx',
      'components/surfaces/Scientia/archiveApi.ts',
      'components/surfaces/Scientia/costRollup.ts',
      'components/surfaces/Scientia/discoveryInboxApi.ts',
      'components/surfaces/Scientia/discoveryReviewApi.ts',
      'components/surfaces/Scientia/noveltyApi.ts',
      'components/surfaces/Search/SearchView.tsx',
      'components/surfaces/Settings/SettingsView.tsx',
      'components/surfaces/SkillsPlugins/SkillsPluginsView.tsx',
      'components/surfaces/SubAgents/subAgentClient.ts',
      'components/surfaces/Tasks/TasksView.tsx',
    ]);

    const unexpected: string[] = [];
    for (const full of collectTsFiles(SRC_ROOT)) {
      const rel = relative(SRC_ROOT, full).replace(/\\/g, '/');
      const text = readFileSync(full, 'utf8');
      if (!text.includes("from '@tauri-apps/api/core'")) continue;
      if (!ALLOW_DIRECT_INVOKE.has(rel)) {
        unexpected.push(rel);
      }
    }
    expect(unexpected).toEqual([]);
  });
});
