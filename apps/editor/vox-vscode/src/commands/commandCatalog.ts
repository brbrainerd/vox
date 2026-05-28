import * as cp from 'child_process';
import * as vscode from 'vscode';

type CatalogTier = 'recommended' | 'advanced' | 'feature_gated';

interface CommandCatalogEntry {
    path: string[];
    command: string;
    about: string;
    aliases: string[];
    has_subcommands: boolean;
    compiled_in: boolean;
    source_group: string;
    feature_gate?: string | null;
    tier: CatalogTier;
}

interface CommandCatalogResult {
    generated_from: string;
    entries: CommandCatalogEntry[];
}

const CACHE_KEY = 'vox.commandCatalog.cache.v1';
const CACHE_STAMP_KEY = 'vox.commandCatalog.cache.stamp.v1';
const CACHE_TTL_MS = 5 * 60 * 1000;
const SAFE_DEFAULTS: CommandCatalogEntry[] = [
    { path: ['build'], command: 'vox build', about: 'Compile a Vox source file, producing TypeScript output', aliases: [], has_subcommands: false, compiled_in: true, source_group: 'fabrica', tier: 'recommended' },
    { path: ['check'], command: 'vox check', about: 'Type-check a Vox source file without producing output', aliases: [], has_subcommands: false, compiled_in: true, source_group: 'fabrica', tier: 'recommended' },
    { path: ['run'], command: 'vox run', about: 'Run a Vox source file (build + cargo run in generated project)', aliases: [], has_subcommands: false, compiled_in: true, source_group: 'fabrica', tier: 'recommended' },
    { path: ['bundle'], command: 'vox bundle', about: 'Bundle a Vox source file into a complete web application', aliases: [], has_subcommands: false, compiled_in: true, source_group: 'fabrica', tier: 'recommended' },
    { path: ['dev'], command: 'vox dev', about: 'Watch and rebuild via vox-compilerd', aliases: [], has_subcommands: false, compiled_in: true, source_group: 'fabrica', tier: 'recommended' },
    { path: ['doctor'], command: 'vox doctor', about: 'Check toolchain and local environment readiness', aliases: [], has_subcommands: false, compiled_in: true, source_group: 'diag', tier: 'recommended' },
    { path: ['completions'], command: 'vox completions', about: 'Emit shell completions for vox', aliases: [], has_subcommands: false, compiled_in: true, source_group: 'fabrica', tier: 'recommended' },
];

export function registerCommandCatalogCommand(context: vscode.ExtensionContext): void {
    context.subscriptions.push(
        vscode.commands.registerCommand('vox.commandPalette', async () => {
            const catalog = await loadCommandCatalog(context);
            const picked = await vscode.window.showQuickPick(
                catalog.entries.map((entry) => ({
                    label: entry.command,
                    description: normalizeAbout(entry.about),
                    detail: buildDetail(entry),
                    entry,
                })),
                {
                    title: 'Vox: Command Entry',
                    placeHolder: 'Type to filter dynamic Vox commands',
                    matchOnDescription: true,
                    matchOnDetail: true,
                },
            );
            if (!picked) {
                return;
            }

            const finalCommand = await vscode.window.showInputBox({
                title: 'Run Vox command',
                prompt: 'Edit command before execution if arguments are needed.',
                value: `${picked.entry.command} `,
            });
            if (!finalCommand || !finalCommand.trim()) {
                return;
            }
            runInVoxTerminal(finalCommand.trim());
        }),
    );
}

async function loadCommandCatalog(context: vscode.ExtensionContext): Promise<CommandCatalogResult> {
    const now = Date.now();
    const cachedStamp = context.globalState.get<number>(CACHE_STAMP_KEY, 0);
    const cached = context.globalState.get<CommandCatalogResult | null>(CACHE_KEY, null);
    if (cached && now - cachedStamp <= CACHE_TTL_MS) {
        return cached;
    }
    const fresh = await fetchCatalogFromCli();
    if (fresh) {
        await context.globalState.update(CACHE_KEY, fresh);
        await context.globalState.update(CACHE_STAMP_KEY, now);
        return fresh;
    }
    if (cached) {
        return cached;
    }
    return {
        generated_from: 'fallback-safe-defaults',
        entries: SAFE_DEFAULTS,
    };
}

function fetchCatalogFromCli(): Promise<CommandCatalogResult | null> {
    return new Promise((resolve) => {
        cp.execFile(
            'vox',
            ['commands', '--format', 'json', '--include-nested'],
            { timeout: 6000, maxBuffer: 1024 * 1024 },
            (err, stdout) => {
                if (err) {
                    resolve(null);
                    return;
                }
                try {
                    const parsed = JSON.parse(stdout) as CommandCatalogResult;
                    if (!parsed.entries || !Array.isArray(parsed.entries)) {
                        resolve(null);
                        return;
                    }
                    resolve(parsed);
                } catch {
                    resolve(null);
                }
            },
        );
    });
}

function buildDetail(entry: CommandCatalogEntry): string {
    const chunks: string[] = [`tier: ${entry.tier}`];
    if (entry.feature_gate) {
        chunks.push(`feature: ${entry.feature_gate}`);
    }
    if (entry.aliases.length > 0) {
        chunks.push(`aliases: ${entry.aliases.join(', ')}`);
    }
    return chunks.join(' | ');
}

function normalizeAbout(about: string): string {
    return about.replace(/\s+/g, ' ').trim();
}

function runInVoxTerminal(command: string): void {
    const existing = vscode.window.terminals.find((t) => t.name === 'Vox');
    const terminal = existing ?? vscode.window.createTerminal('Vox');
    terminal.show();
    terminal.sendText(command);
}

