import { chatSendMessage as transportChatSendMessage } from '../transport';
import type { ChatSendInput, ChatMessageDto } from '../transport';

export type { ChatSendInput, ChatMessageDto } from '../transport';

export interface ParsedChatReply {
  id: string;
  role: 'assistant';
  text: string;
  modelId?: string;
  latencyMs?: number;
  selectionReason?: string;
  createdAt: string;
}

export function parseSendReply(dto: ChatMessageDto): ParsedChatReply {
  return {
    id: String(dto.id),
    role: 'assistant',
    text: dto.content,
    modelId: dto.model_id,
    latencyMs: dto.latency_ms,
    selectionReason: dto.selection_reason,
    createdAt: dto.created_at,
  };
}

/**
 * Calls the real agent loop for a plain chat message and returns the
 * model's reply, already persisted server-side by `chat_send_message`
 * (with a real, non-blank `created_at`). Throws on failure — callers
 * should catch and settle the pending bubble as failed.
 */
export async function sendChatMessage(input: ChatSendInput): Promise<ParsedChatReply> {
  const dto = await transportChatSendMessage(input);
  return parseSendReply(dto);
}
