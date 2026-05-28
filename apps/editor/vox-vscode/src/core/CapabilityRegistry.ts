/** Minimal tool descriptor from MCP `list_tools` */
export interface ListedMcpTool {
    name: string;
    inputSchema?: object;
}

/**
 * Runtime MCP capability map from `list_tools`. Kept in sync on connect/reconnect.
 */
export class CapabilityRegistry {
    private names = new Set<string>();
    private tools: ListedMcpTool[] = [];
    private schemaHash = '';

    /** Last refresh error (e.g. parse), for diagnostics only */
    lastError: string | undefined;

    refreshFromList(tools: ListedMcpTool[]): void {
        this.lastError = undefined;
        this.tools = tools;
        this.names.clear();
        const sink = JSON.stringify(
            tools.map((t) => ({ n: t.name, s: t.inputSchema })).sort((a, b) => (a.n < b.n ? -1 : 1)),
        );
        let h = 0;
        for (let i = 0; i < sink.length; i++) h = (Math.imul(31, h) + sink.charCodeAt(i)) | 0;
        this.schemaHash = (h >>> 0).toString(16);
        for (const t of tools) {
            this.names.add(t.name);
        }
    }

    has(name: string): boolean {
        return this.names.has(name);
    }

    get schemaFingerprint(): string {
        return this.schemaHash;
    }

    /** Count of tools last advertised by the server */
    get size(): number {
        return this.names.size;
    }

    snapshotNames(): string[] {
        return [...this.names].sort();
    }

    /** Names from `required` that were not in the last `list_tools` refresh. */
    missingFromList(required: readonly string[]): string[] {
        return required.filter((n) => !this.names.has(n));
    }
}
