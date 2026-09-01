// The single seam between the composer and the backend. Before this file,
// App.tsx forked on `task_category === 'chat'` and the sync branch mapped four
// fields while the background branch mapped sixteen — so the model picker,
// tier, chips, priority, clutch, risk and dry-run silently did nothing on a
// quick chat. One mapping now, guarded by the key-set assertion.
import { contextRefsFromPayload } from './loquelaContext';
import type { ChatTurnInput } from '../transport';

export const CHAT_TURN_KEYS = [
  'session_id', 'content', 'execution', 'model_override', 'tier',
  'clutch', 'risk', 'context_files', 'active_skill', 'skill_exclusions',
  'grounding_check_enabled', 'priority', 'dry_run', 'allow_duplicate',
  'mode', 'chat_session_id',
] as const;

export interface BuildChatTurnCtx {
  sessionId: string;
  modelOverride?: string | null;
  groundingCheckEnabled?: boolean;
  activeSkillId?: string | null;
  skillExclusions?: string[];
  allowDuplicate?: boolean;
  /** The real originating chat session (e.g. App.tsx's `activeSessionId`),
   *  distinct from `sessionId` above -- which on the background dispatch
   *  path can be a synthetic, throwaway session id (see `newBackgroundSessionId`
   *  call sites in App.tsx). Falls back to `sessionId` when omitted, so
   *  callers that don't set it (the sync path, where the two are the same)
   *  still get a correct value. */
  chatSessionId?: string | null;
}

/** The composer-payload subset the builder reads. Structural rather than
 *  `ChatPayload` so the pure unit test can construct one without the whole
 *  dispatch type. */
export interface ChatTurnSource {
  description: string;
  execution_mode?: 'chat' | 'task' | 'plan';
  model_override?: string | null;
  tier?: string | null;
  clutch?: string | null;
  risk?: string | null;
  active_skill?: string | null;
  priority?: string | null;
  dry_run?: boolean | null;
  /** Interaction mode from the composer (plan|act|verify); forwarded as an
   *  enqueue hint on the background path. */
  mode?: string | null;
  files?: string[];
  context?: unknown;
}

export function buildChatTurn(payload: ChatTurnSource, ctx: BuildChatTurnCtx): ChatTurnInput {
  return {
    session_id: ctx.sessionId,
    content: payload.description,
    execution:
      payload.execution_mode === 'task'
        ? 'background'
        : payload.execution_mode === 'plan'
          ? 'plan'
          : 'sync',
    model_override: payload.model_override ?? ctx.modelOverride ?? null,
    tier: payload.tier ?? null,
    clutch: payload.clutch ?? null,
    risk: payload.risk ?? null,
    context_files: contextRefsFromPayload(payload),
    active_skill: payload.active_skill ?? ctx.activeSkillId ?? null,
    skill_exclusions: ctx.skillExclusions ?? [],
    grounding_check_enabled: ctx.groundingCheckEnabled ?? null,
    priority: payload.priority ?? null,
    dry_run: payload.dry_run ?? null,
    allow_duplicate: ctx.allowDuplicate ?? null,
    mode: payload.mode ?? null,
    chat_session_id: ctx.chatSessionId ?? ctx.sessionId ?? null,
  };
}
