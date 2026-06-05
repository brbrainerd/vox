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
import { test, expect } from '@playwright/test';
import { SURFACE_REGISTRY } from '../src/generated/surfaceRegistry.generated';

/**
 * Every screenshot-able surface is derived from the generated SURFACE_REGISTRY — the same
 * SSOT the sidebar renders from. This keeps the sweep drift-proof: when a surface is added,
 * renamed, combined, or removed, `vox ci gui-surface-registry --write` regenerates the
 * registry and this list follows automatically. There is no hand-maintained view list to
 * fall out of date.
 */
const VIEWS: string[] = Array.from(
  new Set(
    SURFACE_REGISTRY.filter((e) => e.viewKey && e.tier !== 'none').map((e) => e.viewKey as string),
  ),
).sort();

/**
 * Console-error substrings that are environmental noise rather than surface defects
 * (e.g. a missing favicon under the bare Vite dev server). Everything else — React key/prop
 * warnings, failed IPC, render exceptions — fails the audit.
 */
const BENIGN_CONSOLE: string[] = [
  'favicon',
  'Failed to load resource: the server responded with a status of 404',
];

function installMock(target: string) {
  localStorage.setItem('vox_active_view', JSON.stringify(target));
  localStorage.setItem('vox_sidebar_mode', 'default');
  (window as any).__TAURI_CALLS__ = [];

  const modelIds = ['mens-8b', 'opus-4-8', 'sonnet-4-6', 'haiku-4-5', 'qwen-coder-7b', 'local-llama'];
  const modelNames = ['Mens 8B', 'Opus 4.8', 'Sonnet 4.6', 'Haiku 4.5', 'Qwen Coder 7B', 'Local Llama'];
  const models = Array.from({ length: 6 }, (_, i) => ({
    id: modelIds[i],
    // HarnessView keys/reads `model_id` + `display_name`; ModelsView reads `id`. Provide all.
    model_id: modelIds[i],
    display_name: modelNames[i],
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
        case 'get_model_scoreboard': return models.map((m, i) => ({
          model_id: m.id,
          task_category: ['code', 'research', 'chat', 'plan', 'code', 'chat'][i],
          strength_tag: ['speed', 'quality', 'balanced', 'quality', 'speed', 'balanced'][i],
          n_calls: [120, 80, 60, 40, 30, 20][i],
          success_rate: m.success_rate,
          p50_latency_ms: m.latency_p50_ms,
          cost_per_success_usd: [0.0, 0.02, 0.004, 0.001, 0.0, 0.0][i],
          quality_score: m.quality_score,
        }));
        case 'explain_model_selection': return { chosen: 'opus-4-8', reason: 'highest quality within budget' };
        case 'suggest_model_for_task': return 'sonnet-4-6';
        case 'get_ludus_profile': return ludusProfile;
        case 'list_ludus_notifications': return [
          { id: 'n1', level: 'ok', title: 'Level up! → 27', message: 'Reached Centurio', created_at: 1717400000000, kind: 'LevelUp' },
          { id: 'n2', level: 'ok', title: 'Achievement: Bug Slayer', message: 'Fixed 10 bugs', created_at: 1717400000000, kind: 'AchievementUnlocked' },
          { id: 'n3', level: 'warn', title: 'Streak at risk', message: 'Code today to keep your 9-day streak', created_at: 1717400000000, kind: 'StreakLost' },
        ];
        case 'get_gamify_settings': return { enabled: true, mode: 'balanced' };
        case 'list_gamify_leaderboard': return Array.from({ length: 6 }, (_, i) => ({
          rank: i + 1, user_id: ['archon', 'nova', 'cipher', 'quill', 'atlas', 'echo'][i],
          level: [27, 25, 22, 19, 17, 14][i], score: [91000, 84000, 72000, 60000, 51000, 42000][i],
        }));
        case 'list_gamify_companions': return Array.from({ length: 3 }, (_, i) => ({
          id: `comp-${i + 1}`, name: ['Byte', 'Quill', 'Sprocket'][i], description: null,
          language: ['rust', 'typescript', 'python'][i], mood: ['happy', 'focused', 'sleepy'][i],
          health: [80, 65, 40][i], max_health: 100, energy: [70, 50, 30][i], max_energy: 100,
          code_quality: [0.9, 0.8, 0.7][i], last_active: 1717400000000,
          svg: '<svg viewBox="0 0 32 32"><circle cx="16" cy="16" r="14" fill="#d4af37"/></svg>',
        }));
        case 'list_gamify_quests': return Array.from({ length: 3 }, (_, i) => ({
          id: `quest-${i + 1}`, quest_type: ['daily', 'weekly', 'epic'][i],
          description: ['Fix 3 failing tests', 'Land a refactor PR', 'Ship a new surface'][i],
          hint: ['run vox test', 'keep diffs small', 'register it in the surface registry'][i],
          target: [3, 1, 1][i], progress: [2, 0, 1][i], xp_reward: [150, 400, 1000][i],
          crystal_reward: [10, 40, 120][i], completed: [false, false, true][i],
          status: ['active', 'active', 'completed'][i], expires_at: 1717999999999,
        }));
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
      const pageErrors: string[] = [];
      page.on('console', m => { if (m.type() === 'error') consoleErrors.push(m.text()); });
      page.on('pageerror', e => pageErrors.push(e.message));
      await page.addInitScript(installMock, view);
      await page.goto('/');
      // The app shell (sidebar nav) must mount before we judge the surface itself.
      await page.waitForSelector('nav', { timeout: 15_000 });
      await page.waitForTimeout(1600);
      await page.screenshot({ path: `e2e/screens/${view}.png`, fullPage: true });

      // ── Visual-audit assertions ─────────────────────────────────────────
      // 1. The surface rendered without tripping its error boundary.
      await expect(
        page.locator('[data-surface-error]'),
        `[${view}] crashed into its error boundary`,
      ).toHaveCount(0);
      // 2. No uncaught exceptions during render.
      expect(pageErrors, `[${view}] uncaught page errors:\n${pageErrors.join('\n')}`).toEqual([]);
      // 3. No console errors (React key/prop warnings, failed IPC, …) beyond the benign allowlist.
      const meaningful = consoleErrors.filter(t => !BENIGN_CONSOLE.some(b => t.includes(b)));
      expect(meaningful, `[${view}] console errors:\n${meaningful.join('\n')}`).toEqual([]);

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

  // Capture the sidebar in its non-default widths so the grouped/collapsed rendering is audited
  // in every mode, not just 'default'.
  for (const sbMode of ['rail', 'wide'] as const) {
    test(`capture sidebar-${sbMode}`, async ({ browser }) => {
      const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
      const page = await ctx.newPage();
      const pageErrors: string[] = [];
      page.on('pageerror', e => pageErrors.push(e.message));
      await page.addInitScript(installMock, 'dashboard');
      // useLocalStorage JSON-parses its value, so the mode must be stored as JSON.
      await page.addInitScript((m: string) => localStorage.setItem('vox_sidebar_mode', JSON.stringify(m)), sbMode);
      await page.goto('/');
      await page.waitForSelector('nav', { timeout: 15_000 });
      await page.waitForTimeout(1200);
      await page.screenshot({ path: `e2e/screens/_sidebar-${sbMode}.png`, fullPage: false });
      await expect(page.locator('[data-surface-error]')).toHaveCount(0);
      expect(pageErrors, `[sidebar-${sbMode}] uncaught page errors:\n${pageErrors.join('\n')}`).toEqual([]);
      await ctx.close();
    });
  }
});
