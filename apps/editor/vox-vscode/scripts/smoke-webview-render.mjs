import { build } from 'esbuild';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';

const repoRoot = path.resolve(process.cwd());
const tempDir = await mkdtemp(path.join(tmpdir(), 'vox-webview-smoke-'));
const outfile = path.join(tempDir, 'smoke-webview-render.cjs');
const require = createRequire(import.meta.url);

const entry = `
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { ComposerPanel } from './webview-ui/src/components/ComposerPanel.tsx';
import { ContextExplorer } from './webview-ui/src/components/ContextExplorer.tsx';
import { UnifiedDashboard } from './webview-ui/src/components/UnifiedDashboard.tsx';

function assertContains(markup, needle, label) {
  if (!markup.includes(needle)) {
    throw new Error(label + ' missing expected text: ' + needle);
  }
}

const composerMarkup = renderToStaticMarkup(
  React.createElement(ComposerPanel, {
    composerState: {
      availableFiles: ['src/lib.rs', 'README.md'],
      drafts: [
        {
          path: 'src/lib.rs',
          language: 'rust',
          original: 'fn before() {}',
          proposed: 'fn after() {}',
          explanation: 'updated function',
          tokens: 42,
          model_used: 'test-model'
        }
      ],
      isGenerating: false,
      lastPrompt: 'Improve the API',
      snapshotRequested: true,
      lastError: null
    }
  })
);
assertContains(composerMarkup, 'Composer Review', 'composer');
assertContains(composerMarkup, 'Draft Queue', 'composer');

const contextMarkup = renderToStaticMarkup(
  React.createElement(ContextExplorer, {
    inspector: {
      activeEditor: {
        filePath: 'src/main.ts',
        line: 14,
        selectedText: 'const x = 1;',
        languageId: 'typescript',
        diagnostics: [{ severity: 'warning', line: 14, message: 'sample warning' }]
      },
      openFiles: ['src/main.ts', 'src/app.ts'],
      repoIndexStatus: { files_total: 12 },
      repoCatalog: { repositories: [] },
      repoQueryResult: { result_count: 2 },
      capabilityManifest: { capabilities: ['mcp.vox_repo_query_text'] },
      contextKeys: ['workspace_index_status'],
      contextValue: 'cached',
      lastPlan: {
        goal: 'Improve planning',
        tasks: [{ id: 1 }],
        summary: 'summary',
        plan_md: '# Plan',
        written_to_disk: false,
        plan_adequacy_score: 0.81,
        plan_too_thin: false,
        adequacy_reason_codes: ['verification_present']
      },
      lastChatMeta: {
        model_used: 'test-model',
        tokens: 11,
        socrates: {
          risk_decision: 'answer',
          confidence_estimate: 0.9,
          contradiction_ratio: 0.02
        },
        retrieval: {
          retrieval_tier: 'hybrid',
          evidence_count: 3
        }
      },
      browserState: {
        pageId: 7,
        url: 'https://example.com',
        lastAction: 'open'
      },
      lastUpdatedAt: Date.now()
    }
  })
);
assertContains(contextMarkup, 'Evidence And Planning', 'context');
assertContains(contextMarkup, 'Browser lab', 'context');

const dashboardMarkup = renderToStaticMarkup(
  React.createElement(UnifiedDashboard, {
    stats: { activeAgents: '3', queueDepth: '4', latency: '120ms', budget: '$0.14' },
    ops: [{ id: '1', description: 'build', agent_id: 0, status: 'Running' }],
    pipeline: { ok: true },
    budgetHistory: [],
    modelList: [],
    ludusSnapshot: null,
    meshTopology: null
  })
);
assertContains(dashboardMarkup, 'IMPERIUM', 'dashboard');
assertContains(dashboardMarkup, 'Operation Stream', 'dashboard');

console.log('smoke-webview-render: OK');
`;

try {
  await build({
    absWorkingDir: repoRoot,
    bundle: true,
    platform: 'node',
    format: 'cjs',
    target: 'node20',
    outfile,
    jsx: 'automatic',
    stdin: {
      contents: entry,
      loader: 'tsx',
      resolveDir: repoRoot,
      sourcefile: 'smoke-webview-render-entry.tsx',
    },
  });

  require(outfile);
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
