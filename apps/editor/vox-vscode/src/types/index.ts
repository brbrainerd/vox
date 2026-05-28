export * from './attention';

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

/** Orchestrator `vox_config_get` payload shape (see VoxConfig). */
export interface VoxConfigResponse {
    model: string;
    daily_budget_usd: number;
    per_session_budget_usd: number;
    data_dir: string;
    model_dir: string;
    db_url: string | null;
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
    /** Numeric orchestrator agent id when known (retire/pause/drain). */
    orchestrator_agent_id?: number;
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
    /** Wire/session shape from vox-mcp */
    context_files?: string[];
    isStreaming?: boolean;
    is_streaming?: boolean;
    model_used?: string;
}

export interface ChatSocratesMeta {
    risk_decision?: string;
    confidence_estimate?: number;
    contradiction_ratio?: number;
    retrieval_tier?: string;
    contradiction_count?: number;
    evidence_count?: number;
    [key: string]: unknown;
}

export interface ChatRetrievalMeta {
    retrieval_tier?: string;
    contradiction_count?: number;
    evidence_count?: number;
    query_id?: string;
    supporting_claim_ids?: string[];
    contradiction_hints?: string[];
    [key: string]: unknown;
}

export interface ChatSessionMeta {
    model_used?: string;
    tokens?: number;
    session_id?: string;
    socrates?: ChatSocratesMeta | null;
    retrieval?: ChatRetrievalMeta | null;
}

export interface ComposerDraft {
    path: string;
    language: string;
    original: string;
    proposed: string;
    explanation?: string;
    tokens?: number;
    model_used?: string;
}

export interface ComposerState {
    availableFiles: string[];
    drafts: ComposerDraft[];
    isGenerating: boolean;
    lastPrompt?: string;
    lastAppliedPaths?: string[];
    snapshotRequested?: boolean;
    lastError?: string | null;
}

export interface WorkspaceInspectorState {
    activeEditor: ActiveEditorContextView;
    openFiles: string[];
    repoIndexStatus?: unknown;
    repoCatalog?: unknown;
    repoQueryResult?: unknown;
    capabilityManifest?: unknown;
    contextKeys: string[];
    contextValue?: unknown;
    lastPlan?: PlanGoalResult | null;
    lastChatMeta?: ChatSessionMeta | null;
    browserState?: BrowserInspectorState | null;
    lastUpdatedAt?: number;
}

export interface ActiveEditorContextView {
    filePath: string;
    line: number;
    selectedText: string;
    languageId: string;
    diagnostics: Array<{ severity: 'error' | 'warning'; line: number; message: string; source?: string }>;
}

export interface BrowserInspectorState {
    pageId?: string | number | null;
    url?: string | null;
    lastText?: string | null;
    lastExtract?: unknown;
    lastScreenshotPath?: string | null;
    lastAction?: string | null;
    lastError?: string | null;
}

export interface PlanGoalResult {
    goal: string;
    tasks: unknown[];
    summary: string;
    plan_md: string;
    written_to_disk: boolean;
    plan_total_tasks?: number;
    plan_page_offset?: number;
    loop_mode_effective?: string;
    refinement_rounds?: number;
    loop_stop_reason?: string;
    last_aggregate_gap_risk?: number;
    gap_report?: unknown;
    plan_adequacy_score?: number;
    plan_too_thin?: boolean;
    adequacy_reason_codes?: string[];
    plan_depth_effective?: string;
    clarifying_questions?: string[];
    socrates?: ChatSocratesMeta | null;
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

export interface LanguageSurface {
    keywords: string[];
    decorators: string[];
    types: string[];
    builtins: string[];
}

export interface DecoratorMetadata {
    name: string;
    desc: string;
    args?: string | null;
}

export interface BuiltinMetadata {
    name: string;
    sig: string;
    doc: string;
}

export interface AstNode {
    type: string;
    span: { start: number; end: number };
    [key: string]: any;
}

export interface WebviewMessage {
    type: string;
    value?: unknown;
}

/** Matches `vox_mcp::SubmitShadowAdequacy` (shadow plan-adequacy on direct submit). */
export interface SubmitShadowAdequacy {
    score: number;
    is_too_thin: boolean;
    reason_codes: string[];
    critical_count: number;
    aggregate_unresolved_risk: number;
}

/** Matches `vox_mcp::SubmitTaskResponse` from `vox_submit_task`. */
export interface SubmitTaskResponse {
    task_id: number;
    agent_id: number;
    prompt_canonicalized?: boolean;
    conflict_warnings?: string[];
    original_prompt_hash?: string;
    orchestration_contract?: string;
    shadow_plan_adequacy?: SubmitShadowAdequacy;
}
