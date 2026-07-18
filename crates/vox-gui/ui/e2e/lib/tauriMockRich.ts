// crates/vox-gui/ui/e2e/lib/tauriMockRich.ts
/**
 * Dense, overflow-shaped mock for the review-bundle capture matrix, layered
 * over installTauriMock. Sparse mocks are why occlusion/clipping never
 * showed: 44 tasks with 120+-char unicode/RTL titles, 32 models, 6
 * providers, and a dense msgpack orchestrator snapshot make truncation,
 * overlap, and z-fighting visible.
 *
 * Serialization contract (mirrors tauriMockShared.mockInitScript):
 * addInitScript serialises function SOURCE only, so richMockInitScript()
 * composes one self-contained script string:
 *   1. mockInitScript(installTauriMock, viewKey)   // shared + base mock
 *   2. window.__VOX_RICH_BUILD__      = <buildRichDataset source>
 *   3. window.__VOX_RICH_STATUS_BIN__ = new Uint8Array([...])  // msgpack,
 *      encoded HERE in Node — @msgpack/msgpack can't run inside the page
 *   4. (<installTauriMockRich source>)(viewKey)    // wraps base invoke
 */
import type { Page } from '@playwright/test';
import { encode } from '@msgpack/msgpack';
import { mockInitScript } from './tauriMockShared';
import { installTauriMock } from './tauriMock';

/** Self-contained (no captured module scope) — stringified into the page. */
export function buildRichDataset() {
  const long = (s: string, n: number) => s.repeat(Math.ceil(n / s.length)).slice(0, n);
  const hopperTasks = Array.from({ length: 44 }, (_, i) => ({
    item_id: `hop-rich-${i + 1}`,
    intent:
      i % 7 === 0
        ? long(`Refactor the international pipeline № ${i} — очень длинное название задачи с юникодом `, 140)
        : i % 5 === 0
          ? `משימה ${i} — bidirectional text sample with a fairly long tail describing acceptance criteria in detail`
          : long(`Task ${i}: implement, verify, and document the surface behavior across viewports `, 120 + (i % 40)),
    priority: i % 3,
    state: i % 6 === 0 ? 'done' : i % 4 === 0 ? 'assigned' : 'inbox',
    task_id: 9100 + i,
    session_id: i % 2 ? `gui-rich-${i}` : null,
    agent_id: i % 3 ? `agent-${i}` : null,
    remote_node: i % 5 === 0 ? 'node-remote-very-long-hostname.example.internal' : null,
  }));
  const chatSessions = Array.from({ length: 14 }, (_, i) => ({
    session_id: `rich-session-${i + 1}`,
    title: long(`Session ${i + 1}: exploratory conversation about the architecture refactor and its long-term implications `, 90 + i * 4),
    message_count: 3 + i * 7,
    updated_at: 'now',
    conversation_id: i + 1,
  }));
  const models = Array.from({ length: 32 }, (_, i) => {
    const id = i % 5 === 0
      ? ['ollama/llama3-rich-', 'mens/finetune-rich-', 'mesh/node-model-rich-'][i % 3] + i
      : `provider-${i % 6}/model-family-name-${i}-with-a-rather-long-suffix-v${i}.${i % 10}`;
    return {
      id, model_id: id,
      display_name: long(`Model Family ${i} Extended Display Name `, 40 + (i % 30)),
      provider: ['openai', 'anthropic', 'google', 'ollama', 'mistralai', 'meta-llama'][i % 6],
      tier: ['Frontier', 'Fast', 'Budget'][i % 3],
      cost_per_1k: i * 0.0007,
      max_tokens: 8192 * ((i % 4) + 1),
      is_free: i % 8 === 0,
      latency_p50_ms: 200 + i * 13,
      success_rate: 0.9 + (i % 10) / 100,
      quality_score: 0.5 + (i % 50) / 100,
    };
  });
  const providers = Array.from({ length: 6 }, (_, i) => ({
    provider: ['OpenRouter', 'Anthropic', 'OpenAI', 'Ollama', 'Mens Local Inference Cluster (long provider name)', 'Mesh'][i],
    key_present: i !== 2,
    is_local: i >= 3,
    local_reachable: i >= 3 ? i !== 5 : null,
    local_models: i >= 3 ? ['llama3.2', 'qwen-coder-7b', 'mens-8b-instruct-longname'] : [],
  }));
  return { hopperTasks, chatSessions, models, providers };
}

export const RICH_DATASET = buildRichDataset();

/** Dense OrchestratorStatus for dashboard/flow/console. Encoded to msgpack
 * at compose time — NOT inside the page. */
export function buildRichOrchestratorStatus() {
  const agents = Array.from({ length: 9 }, (_, i) => ({
    id: i + 1,
    codename: ['Aquila', 'Bellona', 'Cato', 'Drusus', 'Egeria', 'Faunus', 'Gallus', 'Hersilia', 'Iovis'][i],
    name: `agent-${i + 1}`,
    in_progress: i % 3 !== 0,
    paused: i === 4,
    progress: i % 3 === 0 ? null : ((i * 11) % 100) / 100,
    current_phase: ['plan', 'implement', 'verify', 'review'][i % 4],
    task_description: `Task ${i + 1}: a deliberately long in-flight task description that should truncate or wrap inside the agent card rather than overflow its container boundaries`,
    cost: i * 0.42,
    budget: i % 2 ? 5 : null,
    eta: `${5 + i}m`,
    active_skill: i % 2 ? 'superpowers:test-driven-development' : undefined,
  }));
  const recent_events = Array.from({ length: 24 }, (_, i) => ({
    id: i + 1,
    kind: (['task_started', 'phase_change', 'task_completed', 'doubt_raised'] as const)[i % 4],
    tag: `agent-${(i % 9) + 1}`,
    title: `Event ${i + 1}: ${['started', 'phase → verify', 'completed', 'doubt raised'][i % 4]}`,
    body: 'A stream event body long enough to exercise two-line clamping in the console event feed rendering path.',
    timestamp: 'now',
  }));
  return {
    agent_count: agents.length,
    total_queued: 44, total_in_progress: 6, total_completed: 128, total_doubted: 3,
    total_weighted_load: 7.5, predicted_load: 8.2,
    agents, recent_events,
    alerts: [
      { id: 'al-1', level: 'warn', title: 'Budget 80% consumed', body: 'Exploration spend approaching the configured cap.' },
      { id: 'al-2', level: 'ok', title: 'Mesh healthy', body: 'All peers reachable.' },
    ],
    peers: [
      { id: 'node-a', status: 'online' },
      { id: 'node-b', status: 'online' },
      { id: 'node-remote-very-long-hostname.example.internal', status: 'degraded' },
    ],
    total_cost: 12.34, budget_cap: 50, mesh_throughput: 3.2,
  };
}

/** Self-contained installer: runs AFTER installTauriMock in the same init
 * script; wraps the base invoke and overrides only the dense commands. */
export function installTauriMockRich(viewKey: string): void {
  const internals = (window as any).__TAURI_INTERNALS__;
  const base: ((cmd: string, args?: any) => Promise<unknown>) | undefined = internals?.invoke;
  const build = (window as any).__VOX_RICH_BUILD__;
  const statusBin = (window as any).__VOX_RICH_STATUS_BIN__;
  if (typeof base !== 'function' || typeof build !== 'function' || !statusBin) {
    throw new Error('installTauriMockRich must be injected via addRichMockInitScript after installTauriMock');
  }
  void viewKey; // navigation is seeded by installTauriMock
  const data = build();
  internals.invoke = async (cmd: string, args?: any) => {
    switch (cmd) {
      case 'hopper_list':
        return data.hopperTasks.map((t: any) => ({ ...t }));
      case 'chat_list_sessions': {
        const limit = typeof args?.limit === 'number' ? args.limit : data.chatSessions.length;
        return data.chatSessions.slice(0, limit).map((s: any) => ({ ...s }));
      }
      case 'list_model_cards':
        return data.models;
      case 'inference_provider_status':
        return data.providers;
      case 'get_gamify_settings':
        return { enabled: true, mode: 'balanced' };
      case 'get_orchestrator_status_bin':
        return statusBin;
      default:
        return base(cmd, args);
    }
  };
}

/** Compose the full self-contained init script (exported for unit tests). */
export function richMockInitScript(viewKey: string): string {
  const statusBytes = Array.from(encode(buildRichOrchestratorStatus())).join(',');
  return [
    mockInitScript(installTauriMock, viewKey),
    `window.__VOX_RICH_BUILD__ = ${buildRichDataset.toString()};`,
    `window.__VOX_RICH_STATUS_BIN__ = new Uint8Array([${statusBytes}]);`,
    `(${installTauriMockRich.toString()})(${JSON.stringify(viewKey)});`,
  ].join('\n');
}

/** The ONLY supported way to inject the rich mock into a Playwright page. */
export async function addRichMockInitScript(page: Page, viewKey: string): Promise<void> {
  await page.addInitScript({ content: richMockInitScript(viewKey) });
}
