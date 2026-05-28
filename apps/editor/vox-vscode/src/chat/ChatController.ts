// Thin chat controller — all history, context injection and LLM generation live in Rust (vox-mcp).
// TypeScript only: renders messages, captures user input, forwards to MCP, and relays session metadata.

import type { ChatMessage, ChatSessionMeta } from '../types';
import { VoxMcpClient } from '../core/VoxMcpClient';

export type { ChatMessage };

/** Default sidebar chat session; must match `submitMessage` so history reload sees the same transcript. */
export const SIDEBAR_CHAT_SESSION_ID = 'vscode-sidebar';

export class ChatController {
    private _onUpdate: (messages: ChatMessage[], meta?: ChatSessionMeta | null) => void;

    constructor(
        private readonly _mcp: VoxMcpClient,
        private readonly _chatSessionId: string,
        onUpdate: (messages: ChatMessage[], meta?: ChatSessionMeta | null) => void,
    ) {
        this._onUpdate = onUpdate;
    }

    /** Fetch the authoritative history from the Rust session store. */
    async loadHistory(): Promise<void> {
        const result = await this._mcp.chatHistory(this._chatSessionId);
        this._onUpdate(Array.isArray(result) ? result : [], null);
    }

    /** Submit a message. Rust handles @mention expansion, context injection, model selection, and streaming. */
    async submitMessage(payload: {
        prompt: string;
        contextFiles?: string[];
        openFiles?: string[];
        activeFile?: string;
        activeLine?: number;
        selectedText?: string;
        diagnostics?: unknown[];
        sessionId?: string;
        cognitiveProfile?: 'fast' | 'reasoning' | 'creative';
    }): Promise<void> {
        // Push optimistic user bubble immediately so the UI feels instant
        const optimistic: ChatMessage = {
            id: `opt-${Date.now()}`,
            role: 'user',
            content: payload.prompt,
            timestamp: Date.now(),
            context_files: payload.contextFiles ?? [],
        };
        // Also show streaming assistant placeholder
        const streamId = `stream-${Date.now()}`;
        const streamingMsg: ChatMessage = {
            id: streamId,
            role: 'assistant',
            content: '',
            is_streaming: true,
            timestamp: Date.now() + 1,
        };

        // Immediately notify webview with these two pending messages
        const currentHistory = await this._mcp.chatHistory(this._chatSessionId);
        const base = Array.isArray(currentHistory) ? currentHistory : [];
        this._onUpdate([...base, optimistic, streamingMsg], null);

        // Call native MCP tool — Rust resolves @mentions, injects context, queries LLM
        const result = await this._mcp.chatMessage(payload);

        if (result?.history) {
            this._onUpdate(result.history, {
                model_used: result.model_used,
                tokens: result.tokens,
                session_id: result.session_id,
                socrates: result.socrates ?? null,
                retrieval: result.retrieval ?? null,
            });
        } else {
            // Fallback: reload history
            await this.loadHistory();
        }
    }

    async clearHistory(): Promise<void> {
        await this._mcp.sessionReset();
        this._onUpdate([]);
    }
}
