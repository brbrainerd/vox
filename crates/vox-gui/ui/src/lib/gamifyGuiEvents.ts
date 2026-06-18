import { voxTransport, type GuiEventResultDto } from '../transport';

export type { GuiEventResultDto };

export type GamifyGuiHook =
  | 'chat_message_sent'
  | 'task_submitted'
  | 'search_query_executed'
  | 'policy_rule_viewed'
  | 'palette_navigation'
  | 'console_command_success'
  | 'discovery_action_used'
  | 'workflow_completed'
  | 'model_activated'
  | 'approval_decision'
  | 'browser_preview_loaded'
  | 'mesh_dispatch_success'
  | 'isolation_strategy_set'
  | 'isolation_scan_complete'
  | 'harness_redirect_viewed'
  | 'breadcrumb_navigation'
  | 'claim_approved'
  | 'nanopub_built'
  | 'secret_rotated'
  | 'signing_key_rotated'
  | 'orchestrator_first_connect';

type GuiEventResultListener = (result: GuiEventResultDto) => void;

let guiEventResultListener: GuiEventResultListener | null = null;

/** Registers a listener for successful GUI gamify event results (toast queue). */
export function setGamifyGuiEventResultListener(listener: GuiEventResultListener | null): void {
  guiEventResultListener = listener;
}

/** GUI → gamify event router hook (XP math stays in Rust). Returns null when disabled. */
export function recordGamifyGuiEvent(
  eventType: GamifyGuiHook | string,
  metadata?: Record<string, unknown>,
  options?: { enabled?: boolean },
): Promise<GuiEventResultDto | null> {
  if (options?.enabled === false) return Promise.resolve(null);
  return voxTransport
    .recordGuiEvent(eventType, metadata)
    .then((result) => {
      if (result.xpGranted > 0 || result.lumensGranted > 0 || result.achievementTitle) {
        guiEventResultListener?.(result);
      }
      return result;
    })
    .catch(() => null);
}
