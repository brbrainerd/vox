import { invoke } from '@tauri-apps/api/core';

export interface ChatSendInput {
  session_id: string;
  content: string;
  active_skill?: string | null;
}

export interface ChatMessageDto {
  id: number;
  role: string;
  content: string;
  created_at: string;
  task_id: string | null;
  model_id?: string;
}

export interface ParsedChatReply {
  id: string;
  role: 'assistant';
  text: string;
  modelId?: string;
  createdAt: string;
}

export function parseSendReply(dto: ChatMessageDto): ParsedChatReply {
  return {
    id: String(dto.id),
    role: 'assistant',
    text: dto.content,
    modelId: dto.model_id,
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
  const dto = await invoke<ChatMessageDto>('chat_send_message', { input });
  return parseSendReply(dto);
}
