/**
 * Tauri invoke mock for operator-shell e2e (dashboard, policies, status bar, chat, palette).
 *
 * Injected via `page.addInitScript(installOperatorShellMock, opts)`.
 */
export interface OperatorShellMockOptions {
  initialView?: string;
  seedGuiPrefs?: Record<string, string>;
}

export function installOperatorShellMock(opts: OperatorShellMockOptions = {}): void {
  const initialView = opts.initialView ?? 'dashboard';
  try {
    localStorage.setItem(
      'vox_workbench_tabs.v1',
      JSON.stringify({
        openTabs: Array.from(new Set(['chat', initialView])),
        activeTab: initialView,
      }),
    );
    localStorage.setItem('vox_sidebar_mode', 'default');
  } catch {
    // sandboxed contexts may deny localStorage
  }

  (window as any).__TAURI_CALLS__ = [];
  (window as any).__GUI_PREFS__ = { ...(opts.seedGuiPrefs ?? {}) };
  (window as any).__TAURI_EVENT_HANDLERS__ = {} as Record<string, Set<(...args: unknown[]) => void>>;

  (window as any).__TAURI_INTERNALS__ = {
    transformCallback: (cb: (...args: unknown[]) => unknown) => {
      const id = `cb_${Math.random().toString(36).slice(2)}`;
      (window as any)[id] = cb;
      return id;
    },
    invoke: async (cmd: string, args?: Record<string, unknown>) => {
      (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
      const prefs = (window as any).__GUI_PREFS__ as Record<string, string>;

      switch (cmd) {
        case 'get_gui_preference':
          return prefs[args?.key as string] ?? null;
        case 'set_gui_preference':
          prefs[args?.key as string] = args?.value as string;
          return null;
        // Non-fresh-install defaults so the first-run OnboardingWizard (Task 15)
        // doesn't cover the shell in every operator-shell-mock-backed spec.
        case 'list_secret_status':
          return [{ id: 'OPENROUTER_API_KEY', isPresent: true }];
        case 'inference_provider_status':
          return [];
        case 'get_initial_view':
          return initialView;
        case 'get_build_info':
          return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
        case 'get_orchestrator_status_bin':
          return new Uint8Array([0x80]);
        case 'get_orchestrator_status':
          return { agent_count: 0, agents: [], recent_events: [], alerts: [], peers: [] };
        case 'get_identity_summary':
          return { display_name: 'tester@vox', os_user: 'tester' };
        case 'get_command_catalog':
          return { generated_from: 'e2e-mock', entries: [] };
        case 'get_action_manifest':
          return {
            x_vox_version: 2,
            schema_version: 1,
            generated_from: 'e2e-mock',
            actions: [],
          };
        case 'get_routing_summary':
        case 'get_routing_summary_live':
          return {
            active_model: 'mens-8b',
            exploration_spent_usd: 0,
            exploration_budget_usd: 50,
            arm_count: 1,
            model_count: 1,
            decision_preview: null,
          };
        case 'get_active_model':
          return 'mens-8b';
        case 'get_selection_policy':
          return { chain: ['mens-8b'], free_tier: true };
        case 'list_model_cards':
          return [{ model_id: 'mens-8b', display_name: 'Mens 8B', id: 'mens-8b' }];
        case 'get_model_scoreboard':
          return [];
        case 'list_gui_runs':
          return [];
        case 'list_orchestrator_tasks':
          return [];
        case 'get_llm_spend':
          return {
            sessionUsd: 0,
            dayUsd: 0,
            totalUsd: 1.25,
            dailyBudgetUsd: 50,
            perSessionBudgetUsd: 10,
          };
        case 'get_gamify_settings':
          return { enabled: false, mode: 'off' };
        case 'get_ludus_profile':
          return {
            user_id: 'local',
            level: 1,
            xp: 0,
            xp_to_next_level: 100,
            xp_progress: 0,
            total_xp_earned: 0,
            crystals: 0,
            lumens: 0,
            energy: 100,
            max_energy: 100,
            current_streak: 0,
            prestige_level: 0,
            title: 'Initiate',
            full_title: 'Initiate',
            trust_tier: 'New',
          };
        case 'list_ludus_notifications':
          return [];
        case 'record_gui_event':
          return { xpGranted: 0, lumensGranted: 0, achievementTitle: null };
        case 'vox_docs_index':
          return [];
        case 'read_doc_markdown':
          return `# ${String(args?.path ?? 'doc')}\n\nMock doc.`;
        case 'vox_search_query':
          return { hits: [], next_cursor: null, total: 0 };
        case 'chat_list_sessions':
          return [
            {
              session_id: 'mock-session-1',
              title: 'Mock chat',
              updated_at: 'now',
              message_count: 0,
              conversation_id: 1,
            },
          ];
        case 'chat_create_session':
          return {
            session_id: 'mock-session-new',
            title: 'New chat',
            updated_at: 'now',
            message_count: 0,
            conversation_id: 2,
          };
        case 'chat_get_messages':
          return [];
        case 'policy_list':
          return [
            {
              id: 'fmt.rust',
              title: 'Rust formatting',
              domain: 'format',
              group: 'Formatting',
            },
          ];
        case 'list_branches':
          return [{ branch: 'main', path: '.', isCurrent: true }];
        case 'policy_status':
          return [];
        case 'policy_show':
          return {
            id: 'fmt.rust',
            title: 'Rust formatting',
            domain: 'format',
            description: 'x',
            blocking: true,
            runsOn: ['push'],
            origin: 'builtin',
            sourceKind: 'lint',
            sourceRef: 'r',
          };
        case 'invoke_mcp_tool': {
          const tool = String(args?.tool ?? '');
          if (tool.includes('pending_approval')) {
            return {
              tool,
              is_error: false,
              result: { success: true, data: { approvals: [] } },
            };
          }
          if (tool.includes('skill')) {
            return { tool, is_error: false, result: { success: true, data: { skills: [] } } };
          }
          return { tool, is_error: false, result: { success: true, data: {} } };
        }
        case 'pty_spawn':
        case 'pty_write':
        case 'pty_close':
          return null;
        case 'execute_command':
          return { exit_code: 0, stdout: 'ok', stderr: '' };
        case 'submit_orchestrator_task':
          return { ok: true, task_id: '101', message: 'submitted' };
        case 'pause_orchestrator_agent':
        case 'resume_orchestrator_agent':
          return { ok: true };
        case 'discovery_record':
          return null;
        default:
          return null;
      }
    },
  };
}
