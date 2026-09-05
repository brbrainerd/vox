import { chatTurn as transportChatTurn } from '../transport';
import type { ChatTurnInput, ChatTurnDto } from '../transport';
import type { TurnEventDto } from '../types/dashboard';

export type { ChatTurnInput, ChatTurnDto } from '../transport';

export interface ParsedChatReply {
  id: string;
  role: 'assistant';
  text: string;
  modelId?: string;
  latencyMs?: number;
  selectionReason?: string;
  createdAt: string;
  /** True when the opt-in post-reply grounding check flagged this reply as
   *  low-confidence (only set when the caller passed `grounding_check_enabled`). */
  groundingFlagged?: boolean;
  /** Turn events derived from tool results this turn (e.g. a skill-activation
   *  chip) — see Rust `turn_event_for_result`. */
  events?: TurnEventDto[];
}

export function parseSendReply(dto: ChatTurnDto): ParsedChatReply {
  return {
    id: String(dto.id),
    role: 'assistant',
    text: dto.content,
    modelId: dto.model_id,
    latencyMs: dto.latency_ms,
    selectionReason: dto.selection_reason,
    createdAt: dto.created_at,
    groundingFlagged: dto.grounding_flagged,
    events: dto.events,
  };
}

/**
 * Calls the real agent loop for a synchronous chat turn and returns the
 * model's reply, already persisted server-side by `chat_turn` (with a real,
 * non-blank `created_at`). Throws on failure — callers should catch and
 * settle the pending bubble as failed.
 */
export async function sendChatTurn(input: ChatTurnInput): Promise<ParsedChatReply> {
  const dto = await transportChatTurn(input);
  return parseSendReply(dto);
}
