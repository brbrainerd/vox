/**
 * Shared MCP tool result parsing (MCP content blocks + vox-mcp ToolResult envelope).
 */

export type McpLogSink = { appendLine(line: string): void };

interface McpResult {
    content: Array<{ type: string; text: string }>;
}

export function parseMcpToolResult(result: unknown): unknown {
    const r = result as McpResult | null;
    if (!r || !r.content || r.content.length === 0) return null;
    const text = r.content[0]?.type === 'text' ? r.content[0].text : '{}';
    try {
        return JSON.parse(text);
    } catch {
        return text;
    }
}

/** Unwrap Rust `ToolResult<T>` JSON (`success` / `data` / `error`). */
export function unwrapVoxToolEnvelope(
    parsed: unknown,
    log: McpLogSink,
    toolName: string,
): unknown {
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed) && 'success' in parsed) {
        const o = parsed as { success?: boolean; data?: unknown; error?: string };
        if (o.success === false) {
            if (o.error) log.appendLine(`[Vox MCP] Tool failed [${toolName}]: ${o.error}`);
            return null;
        }
        if ('data' in o && o.data !== undefined) return o.data;
    }
    return parsed;
}
