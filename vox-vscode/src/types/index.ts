// Shared types for the vox-vscode extension

export interface ProviderStatus {
    provider: string;
    model: string;
    configured: boolean;
    calls_used: number;
    daily_limit: number;
    remaining: number;
}

export interface VoxStatus {
    providers: ProviderStatus[];
    cost_today_usd: number;
}

export type AgentRole = 'build' | 'plan' | 'debug' | 'research' | 'review' | 'unknown';
export type AgentStatus = 'idle' | 'working' | 'done' | 'error' | 'waiting';
export type AgentMode = 'chat' | 'plan' | 'debug' | 'auto';

export interface AgentEvent {
    id?: string;
    agent_id: string;
    event_type: string;
    type?: string;
    payload?: string;
    message?: string;
    timestamp?: number;
    data?: unknown;
}

export interface AgentState {
    agent_id: string;
    role: AgentRole;
    status: AgentStatus;
    current_task?: string;
    xp?: number;
    mood?: string;
    last_event: AgentEvent;
    last_seen: number;
}

export interface Snapshot {
    id: string;
    message: string;
    timestamp: number;
    kind: 'code' | 'db' | 'full';
    files_changed?: number;
}

export interface OplogEntry {
    id: string;
    op_type: string;
    description: string;
    timestamp: number;
    reversible: boolean;
}

export interface ModelMetadata {
    id: string;
    displayName: string;
    provider: 'google' | 'openrouter' | 'ollama' | 'anthropic' | 'openai' | 'groq' | 'together';
    tier: 'free' | 'byok' | 'local';
    requestsPerMinute?: number;
    requestsPerDay?: number;
    contextWindow: number;
    costPer1MTok?: number;
    tags: string[];
    icon: string;
    description: string;
}

export interface GamifyState {
    agent_id?: string;
    level: number;
    xp: number;
    crystals: number;
    streak: number;
    streak_frozen: boolean;
    companion_name?: string;
    companion_mood?: string;
    achievements?: Achievement[];
}

export interface Achievement {
    id: string;
    name: string;
    description: string;
    icon: string;
    unlocked_at?: number;
}

export interface BudgetStatus {
    total_tokens_used?: number;
    total_cost_usd?: number;
    providers?: ProviderStatus[];
}

export interface FileChange {
    path: string;
    old_content: string;
    new_content: string;
    language: string;
}

export interface ComposerTask {
    id: string;
    prompt: string;
    contextFiles: string[];
    status: 'pending' | 'running' | 'awaiting_review' | 'done' | 'error';
    changes: FileChange[];
    agentId?: string;
    createdAt: number;
    tokens?: number;
    costUsd?: number;
}

export interface ChatMessage {
    id: string;
    role: 'user' | 'assistant' | 'system';
    content: string;
    timestamp: number;
    tokens?: number;
    contextFiles?: string[];
    isStreaming?: boolean;
}

export interface SkillInfo {
    id: string;
    name: string;
    version: string;
    description: string;
    author?: string;
    installed: boolean;
    tags?: string[];
}

export interface WebviewMessage {
    type: string;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    value?: any;
}
