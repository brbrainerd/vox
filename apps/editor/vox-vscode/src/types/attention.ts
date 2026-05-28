export interface AttentionStatusPayload {
    enabled?: boolean;
    spent_ms?: number;
    max_ms?: number;
    spent_ratio?: number;
    focus_depth?: string;
    interrupt_freq_per_hour?: number;
    budget_signal?: string;
    auto_approve_ratio?: number;
    efficiency?: number;
    exhausted?: boolean;
    alert_threshold?: number;
    trust_scores?: unknown[];
    tlx_weights?: Record<string, number>;
    tier_gate?: Record<string, unknown>;
    interruption_calibration?: Record<string, number>;
    inbox_suppressed_count?: number;
}

export interface AttentionHistoryParams {
    since_hours?: number;
    channel?: string;
    agent_id?: number;
    limit?: number;
}

export interface AttentionHistoryPayload {
    events: unknown[];
}
