/**
 * Visual-audit screenshot sweep.
 *
 * Extends the existing dashboard.spec Tauri-bridge mock (window.__TAURI_INTERNALS__.invoke)
 * into a full capture of every GUI surface. For each view it spins up a fresh page with the
 * target view pre-selected (localStorage + get_initial_view) and a rich invoke mock so panels
 * render with representative data, then writes a full-page PNG to e2e/screens/<view>.png.
 *
 * Run: pnpm exec playwright test screenshots.spec.ts --project=chromium
 */
import { test } from '@playwright/test';

const VIEWS = [
  'dashboard', 'flow', 'catalog', 'matrix',
  'scientia', 'claims', 'research', 'publications',
  'mens', 'populi', 'oratio', 'models',
  'harness', 'repository', 'mesh', 'gamify',
  'runs', 'approvals', 'skills', 'memory',
  'search', 'coverage', 'settings',
];

function installMock(target: string) {
  localStorage.setItem('vox_active_view', JSON.stringify(target));
  localStorage.setItem('vox_sidebar_mode', 'default');
  (window as any).__TAURI_CALLS__ = [];

  const models = Array.from({ length: 6 }, (_, i) => ({
    id: ['mens-8b', 'opus-4-8', 'sonnet-4-6', 'haiku-4-5', 'qwen-coder-7b', 'local-llama'][i],
    provider: ['mens', 'anthropic', 'anthropic', 'anthropic', 'local', 'ollama'][i],
    tier: ['Local', 'Elite', 'Pro', 'Fast', 'Free', 'Local'][i],
    cost_per_1k: [0, 0.015, 0.003, 0.0008, 0, 0][i],
    max_tokens: 200000,
    is_free: [true, false, false, false, true, true][i],
    latency_p50_ms: [120, 900, 600, 300, 200, 150][i],
    success_rate: [0.98, 0.99, 0.985, 0.97, 0.95, 0.93][i],
    quality_score: [0.78, 0.95, 0.9, 0.82, 0.7, 0.65][i],
  }));

  const queueSnapshot = {
    candidates: {
      total: 14,
      by_class: { algorithmic_improvement: 5, reproducibility_infra: 4, telemetry_trust: 3, policy_governance: 2 },
      top_5_by_confidence: Array.from({ length: 5 }, (_, i) => ({
        candidate_id: `cand-${100 + i}`, candidate_class: 'algorithmic_improvement',
        confidence: 0.9 - i * 0.07, state: 'evidence_incomplete',
        created_at_ms: 1717000000000, updated_at_ms: 1717400000000,
      })),
    },
    claims_pending: { verifiable: 23, abstained: 6, extraction_running: 2 },
    manifests_in_reply_window: ['pub-7', 'pub-9'],
    retraction_queue: ['pub-3'],
    stalls: [{ candidate_id: 'cand-77', class: 'telemetry_trust', stuck_for_ms: 3200000000 }],
  };

  const searchResponse = {
    hits: Array.from({ length: 8 }, (_, i) => ({
      source: ['memory', 'chunk', 'repo', 'web', 'knowledge', 'memory', 'chunk', 'repo'][i],
      kind: ['memory', 'doc', 'code', 'web', 'knowledge', 'memory', 'doc', 'code'][i],
      path: ['MEMORY.md', 'docs/architecture/search.md', 'crates/vox-search/src/execution.rs',
             'https://example.com/hybrid-search', 'node:retrieval', 'feedback_no_stubs.md',
             'docs/spec.md', 'crates/vox-gui/src/commands/search.rs'][i],
      title: ['Memory index', 'Search design', 'execute_search_plan', 'Hybrid Search', 'retrieval node', null, null, null][i],
      snippet: 'the hybrid search engine fuses bm25 and vector recall with rrf over the candidate set',
      score: 0.95 - i * 0.08,
      provenance: ['bm25', 'vector'],
      locator: { kind: ['memory', 'file', 'file', 'web', 'memory', 'memory', 'file', 'file'][i], value: 'x' },
    })),
    facets_by_source: [{ value: 'memory', count: 3 }, { value: 'chunk', count: 2 }, { value: 'repo', count: 2 }, { value: 'web', count: 1 }],
    facets_by_kind: [{ value: 'doc', count: 2 }, { value: 'code', count: 2 }, { value: 'memory', count: 3 }, { value: 'web', count: 1 }],
    total: 23, next_cursor: 8, corpora: ['memory', 'documentchunks', 'repoinventory', 'webresearch'],
  };

  const ludusProfile = {
    user_id: 'local', level: 27, xp: 4200, xp_to_next_level: 800, xp_progress: 0.62,
    total_xp_earned: 91000, crystals: 1840, lumens: 320, energy: 80, max_energy: 120,
    current_streak: 9, prestige_level: 1, title: 'Centurio', full_title: 'Ascendant Centurio', trust_tier: 'Proven',
  };

  const manifests = Array.from({ length: 10 }, (_, i) => ({
    publication_id: `pub-${i + 1}`, content_type: 'paper',
    state: ['draft', 'draft', 'doi_reserved', 'approved', 'approved', 'submitted', 'submitted', 'published', 'published', 'failed'][i],
    created_at_ms: 1717000000000, updated_at_ms: 1717400000000,
  }));

  const sessions = Array.from({ length: 6 }, (_, i) => ({
    id: i + 1, status: ['completed', 'completed', 'failed', 'active', 'completed', 'orphaned'][i],
    query_text: ['vector db tradeoffs', 'rrf fusion weights', 'tantivy vs qdrant', 'embedding drift', 'crag routing', 'eval harness'][i],
    started_at_ms: 1717000000000, finished_at_ms: 1717400000000,
  }));

  const mcpResult = (tool: string) => {
    if (tool.includes('mesh_nodes')) return { nodes: [{ id: 'node-a', status: 'online', vram_gb: 24 }, { id: 'node-b', status: 'online', vram_gb: 12 }], edges: [] };
    if (tool.includes('pending_approval')) return { pending: [{ id: 'appr-1', tool: 'vox_run_shell', args: { cmd: 'rm -rf build' }, requested_at_ms: 1717400000000 }] };
    if (tool.includes('skill') || tool.includes('plugin')) return { skills: [{ id: 'superpowers', name: 'Superpowers', enabled: true }], plugins: [{ id: 'design', name: 'Design' }] };
    return { ok: true };
  };

  (window as any).__TAURI_INTERNALS__ = {
    invoke: async (cmd: string, args?: any) => {
      (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
      switch (cmd) {
        case 'get_initial_view': return target;
        case 'get_build_info': return { version: '0.6.0', display: '0.6.0+local (dev)' };
        case 'get_orchestrator_status_bin': return new Uint8Array([0x80]);
        case 'get_orchestrator_status': return { agent_count: 0, agents: [], recent_events: [], alerts: [], peers: [] };
        case 'list_model_cards': return models;
        case 'get_active_model': return 'opus-4-8';
        case 'get_routing_summary': case 'get_routing_summary_live':
          return {
            active_model: 'opus-4-8', exploration_spent_usd: 2.4, exploration_budget_usd: 50,
            arm_count: 6, model_count: 6,
            decision_preview: { selected_model: 'opus-4-8', discovery_state: 'exploit',
              alternatives: ['sonnet-4-6', 'haiku-4-5'], rejection_reasons: ['budget cap'],
              intelligence_score: 0.92, efficiency_score: 0.7, latency_score: 0.6 },
          };
        case 'get_selection_policy': return { chain: ['opus-4-8', 'sonnet-4-6', 'haiku-4-5'], free_tier: true };
        case 'get_model_scoreboard': return models.map(m => ({ id: m.id, success_rate: m.success_rate, quality_score: m.quality_score, latency_p50_ms: m.latency_p50_ms }));
        case 'explain_model_selection': return { chosen: 'opus-4-8', reason: 'highest quality within budget' };
        case 'suggest_model_for_task': return 'sonnet-4-6';
        case 'get_ludus_profile': return ludusProfile;
        case 'list_ludus_notifications': return [
          { id: 'n1', level: 'ok', title: 'Level up! → 27', message: 'Reached Centurio', created_at: 1717400000000, kind: 'LevelUp' },
          { id: 'n2', level: 'ok', title: 'Achievement: Bug Slayer', message: 'Fixed 10 bugs', created_at: 1717400000000, kind: 'AchievementUnlocked' },
          { id: 'n3', level: 'warn', title: 'Streak at risk', message: 'Code today to keep your 9-day streak', created_at: 1717400000000, kind: 'StreakLost' },
        ];
        case 'get_gamify_settings': return { enabled: true, mode: 'balanced' };
        case 'vox_search_query': return searchResponse;
        case 'open_locator': return { action: 'opened' };
        case 'list_research_sessions': return sessions;
        case 'get_research_session_detail': return { session: sessions[0], report_markdown: '# Findings\n\nVector DBs trade recall for latency...\n\n- qdrant: fast ANN\n- tantivy: lexical', artifact_json: '{}' };
        case 'list_publication_manifests': return manifests;
        case 'get_memory_status': return {
          corpus_counts: { proj: 1280, docs: 540, chats: 96, rules: 210, web: 60 },
          shards: [
            { id: 'proj', depth: 3, entries: 1280, hot: true, dirty: false, spark: [2, 5, 3, 8, 6, 9, 7] },
            { id: 'docs', depth: 2, entries: 540, hot: false, dirty: true, spark: [1, 2, 1, 3, 2, 4, 3] },
            { id: 'chats', depth: 1, entries: 96, hot: false, dirty: false, spark: [0, 1, 0, 2, 1, 1, 2] },
            { id: 'rules', depth: 2, entries: 210, hot: true, dirty: false, spark: [1, 1, 2, 2, 3, 2, 3] },
          ],
        };
        case 'mnemosyne_recall': return sessions.map((s, i) => ({ src: 'memory', line: 0, score: 0.9 - i * 0.1, kind: 'memory', text: s.query_text }));
        case 'get_command_catalog': return {
          generated_from: 'mock',
          entries: ['check', 'build', 'test', 'run', 'fmt', 'audit', 'research', 'scientia'].map(n => ({
            path: [n], command: `vox ${n}`, about: `Run vox ${n}`, aliases: [], has_subcommands: false,
            compiled_in: true, source_group: 'core', feature_gate: null, tier: 'recommended',
            arguments: [{ name: 'path', short: null, long: 'path', help: 'Target path', required: false, takes_value: true, value_kind: 'value', possible_values: [], default_values: [] }],
          })),
        };
        case 'get_action_manifest': return { x_vox_version: 2, schema_version: 1, generated_from: 'mock', actions: [] };
        case 'get_full_registry': return { commands: [] };
        case 'get_command_metadata': return { safety_class: 'read_only', confirmation_policy: 'none' };
        case 'list_gui_runs': return Array.from({ length: 5 }, (_, i) => ({
          run_id: `gui-run-${i + 1}`, workflow_name: ['gui.harness.submit', 'gui.policy.doubt', 'gui.search', 'gui.research', 'gui.repo'][i],
          status: ['success', 'success', 'running', 'failed', 'success'][i], planned_steps: 3, completed_steps: [3, 3, 1, 2, 3][i],
          updated_at_ms: 1717400000000, last_error: i === 3 ? 'exit code 1' : null,
        }));
        case 'get_gui_run': return { run_id: 'gui-run-1', workflow_name: 'gui.harness.submit', status: 'success', steps: [] };
        case 'list_secret_status': return [
          { id: 'ANTHROPIC_API_KEY', present: true, preview: 'sk-...abcd' },
          { id: 'OPENROUTER_API_KEY', present: false, preview: null },
          { id: 'TAVILY_API_KEY', present: true, preview: 'tvly-...wxyz' },
        ];
        case 'get_gui_preference': return null;
        case 'invoke_mcp_tool': return { tool: args?.tool ?? 'unknown', is_error: false, result: mcpResult(args?.tool ?? '') };
        case 'execute_command': {
          const path: string[] = args?.path ?? [];
          const p = path.join(' ');
          if (p === 'scientia dashboard') return { exit_code: 0, stdout: JSON.stringify(queueSnapshot), stderr: '' };
          if (p === 'scientia claims' || p === 'scientia publication-extract-claims')
            return { exit_code: 0, stdout: JSON.stringify({ claims: Array.from({ length: 5 }, (_, i) => ({ claim_id: `c${i}`, text: 'Provider X shows 3% regression under load', verdict: ['Supported', 'Contested', 'Abstain', 'Supported', 'Contradicted'][i], confidence: 0.8 - i * 0.1, verifiability_score: 0.7, numeric: true, verifier_model: 'minicheck' })) }), stderr: '' };
          if (path[0] === 'research') return { exit_code: 0, stdout: 'SearXNG: ok\nDDG: ok\nTavily: ok', stderr: '' };
          if (path[0] === 'mens') return { exit_code: 0, stdout: 'training idle | 2 local models | GPU: RTX 4090 (24GB)', stderr: '' };
          if (path[0] === 'populi') return { exit_code: 0, stdout: 'mesh: 2 nodes online | overlay healthy', stderr: '' };
          if (path[0] === 'oratio') return { exit_code: 0, stdout: 'oratio runtime ok | backend: whisper-local', stderr: '' };
          return { exit_code: 0, stdout: 'ok', stderr: '' };
        }
        case 'submit_orchestrator_task': return { ok: true, task_id: '101', message: 'submitted' };
        default: return null;
      }
    },
  };
}

test.describe('GUI visual audit', () => {
  for (const view of VIEWS) {
    test(`capture ${view}`, async ({ browser }) => {
      const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
      const page = await ctx.newPage();
      const consoleErrors: string[] = [];
      page.on('console', m => { if (m.type() === 'error') consoleErrors.push(m.text()); });
      page.on('pageerror', e => consoleErrors.push('PAGEERROR: ' + e.message));
      await page.addInitScript(installMock, view);
      await page.goto('/');
      await page.waitForTimeout(1600);
      await page.screenshot({ path: `e2e/screens/${view}.png`, fullPage: true });
      if (consoleErrors.length) {
        // Surface console errors into the test output for the audit.
        console.log(`[${view}] console errors:\n` + consoleErrors.slice(0, 12).join('\n'));
      }
      await ctx.close();
    });
  }

  test('capture command-palette', async ({ browser }) => {
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await ctx.newPage();
    await page.addInitScript(installMock, 'dashboard');
    await page.goto('/');
    await page.waitForTimeout(1000);
    await page.keyboard.press('Control+k');
    await page.waitForTimeout(400);
    await page.keyboard.type('search');
    await page.waitForTimeout(600);
    await page.screenshot({ path: 'e2e/screens/_command-palette.png', fullPage: false });
    await ctx.close();
  });
});
