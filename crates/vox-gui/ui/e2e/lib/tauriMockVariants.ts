/**
 * Variant Tauri-invoke mocks for multi-state visual audit screenshot sweeps.
 *
 * installEmptyStateMock  — all list/count IPC commands return empty; detail commands return null.
 * installErrorStateMock  — key data-fetch IPC commands throw so the UI must render error states.
 *
 * Usage (Playwright):
 *   await page.addInitScript(installEmptyStateMock, viewKey)
 *   await page.addInitScript(installErrorStateMock, viewKey)
 *
 * Structurally mirrors installTauriMock in tauriMock.ts — drop-in replacement.
 */

/** IPC commands that return lists — return [] in empty-state mock. */
const LIST_CMDS = new Set([
  'list_model_cards', 'list_gui_runs', 'list_ludus_notifications',
  'list_gamify_leaderboard', 'list_gamify_companions', 'list_gamify_quests',
  'list_research_sessions', 'list_publication_manifests', 'list_branches',
  'list_secret_status', 'list_repo_files', 'chat_list_sessions',
  'policy_list', 'policy_status', 'get_routing_intentions', 'get_model_scoreboard',
  'list_orchestrator_tasks',
]);

/**
 * Return a typed-empty response for detail commands (not null) so the UI
 * doesn't crash on destructuring (e.g. `const { hits } = result` when result is null).
 */
function emptyDetailResponse(cmd: string): unknown {
  switch (cmd) {
    case 'get_memory_status': return { corpus_counts: {}, shards: [] };
    case 'get_command_catalog': return { generated_from: 'mock-empty', entries: [] };
    case 'vox_search_query': return { hits: [], facets_by_source: [], facets_by_kind: [], total: 0, next_cursor: null, corpora: [] };
    case 'get_routing_summary_live': return { active_model: null, decision_preview: null };
    case 'execute_command': return { exit_code: 0, stdout: '', stderr: '' };
    case 'get_full_registry': return { commands: [] };
    case 'get_completion_report': return { score: 100, warnings: [], is_complete: true };
    case 'get_archive_status': return { swhid: null, swh_task_id: null, swh_task_status: null, zenodo_doi: null, zenodo_state: null };
    default: return null;
  }
}

/** Detail-fetch commands with a known shape — return typed-empty, not null. */
const DETAIL_CMDS = new Set([
  'get_routing_summary_live', 'get_ludus_profile', 'get_research_session_detail',
  'get_memory_status', 'get_command_catalog', 'get_full_registry',
  'get_command_metadata', 'get_gui_run', 'get_task_diff',
  'explain_model_selection', 'suggest_model_for_task', 'vox_search_query', 'execute_command',
  'get_completion_report', 'get_archive_status',
]);

/** Commands that must succeed for the app shell to mount at all. */
function bootstrapResponse(cmd: string, viewKey: string): unknown {
  switch (cmd) {
    case 'get_initial_view': return viewKey;
    case 'get_build_info': return { version: '0.6.0', display: '0.6.0+local (dev)' };
    case 'get_orchestrator_status_bin': return new Uint8Array([0x80]);
    case 'get_orchestrator_status': return { agent_count: 0, agents: [], recent_events: [], alerts: [], peers: [] };
    case 'get_action_manifest': return { x_vox_version: 2, schema_version: 1, generated_from: 'mock-empty', actions: [] };
    case 'get_gui_preference': return null;
    case 'get_gamify_settings': return { enabled: false, mode: 'off' };
    case 'get_identity_summary': return { display_name: 'tester@vox', os_user: 'tester' };
    case 'get_active_model': return null;
    case 'get_selection_policy': return { chain: [], free_tier: true };
    case 'vox_docs_index': return [];
    default: return null;
  }
}

export function installEmptyStateMock(viewKey: string): void {
  try {
    window.localStorage.setItem('vox_active_view', JSON.stringify(viewKey));
    window.localStorage.setItem('vox_sidebar_mode', 'default');
  } catch { /* sandboxed contexts may deny localStorage */ }
  (window as any).__TAURI_CALLS__ = [];
  (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event: string, eventId: number) => {}
  };

  (window as any).__TAURI_INTERNALS__ = {
    ...((window as any).__TAURI_INTERNALS__ || {}),
    transformCallback: (cb: (...args: unknown[]) => unknown) => {
      const id = `cb_${Math.random().toString(36).slice(2)}`;
      (window as any)[id] = cb;
      return id;
    },
    invoke: async (cmd: string, args?: any) => {
      (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
      if (LIST_CMDS.has(cmd)) return [];
      if (DETAIL_CMDS.has(cmd)) return emptyDetailResponse(cmd);
      if (cmd === 'plugin:event|listen') return Math.floor(Math.random() * 10000);
      if (cmd === 'plugin:event|unlisten') return null;
      return bootstrapResponse(cmd, viewKey);
    },
  };
}

/** IPC commands whose failure exercises error-state UI in the component. */
const ERROR_CMDS = new Set([
  'list_gui_runs', 'list_model_cards', 'get_routing_summary_live',
  'vox_search_query', 'list_research_sessions', 'list_publication_manifests',
  'get_memory_status', 'chat_list_sessions', 'get_model_scoreboard',
  'get_ludus_profile', 'policy_list', 'policy_status',
  'list_gamify_companions', 'list_gamify_quests', 'list_gamify_leaderboard',
  'get_command_catalog', 'list_orchestrator_tasks', 'get_archive_status',
  'get_completion_report',
]);

export function installErrorStateMock(viewKey: string): void {
  try {
    window.localStorage.setItem('vox_active_view', JSON.stringify(viewKey));
    window.localStorage.setItem('vox_sidebar_mode', 'default');
  } catch { /* sandboxed contexts may deny localStorage */ }
  (window as any).__TAURI_CALLS__ = [];
  (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event: string, eventId: number) => {}
  };

  (window as any).__TAURI_INTERNALS__ = {
    ...((window as any).__TAURI_INTERNALS__ || {}),
    transformCallback: (cb: (...args: unknown[]) => unknown) => {
      const id = `cb_${Math.random().toString(36).slice(2)}`;
      (window as any)[id] = cb;
      return id;
    },
    invoke: async (cmd: string, args?: any) => {
      (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
      if (ERROR_CMDS.has(cmd)) {
        throw new Error(`[mock-error] ${cmd} simulated IPC failure`);
      }
      if (cmd === 'plugin:event|listen') return Math.floor(Math.random() * 10000);
      if (cmd === 'plugin:event|unlisten') return null;
      return bootstrapResponse(cmd, viewKey);
    },
  };
}
