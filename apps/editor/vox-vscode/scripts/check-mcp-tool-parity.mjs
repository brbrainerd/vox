/**
 * Fail if extension sources reference vox MCP tool names not in the canonical registry
 * (aliases parsed from crates/vox-orchestrator-mcp/src/tool_aliases.rs — SSOT moved
 * from crates/vox-orchestrator/src/mcp_tools/ during the 2026-05-08 reorg Phase 4).
 */
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// scripts/ -> vox-vscode/ -> editor/ -> apps/ -> repository root
const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
const extRoot = path.join(repoRoot, 'apps', 'editor', 'vox-vscode');
const yamlPath = path.join(repoRoot, 'contracts', 'mcp', 'tool-registry.canonical.yaml');
const rustAliasesPath = path.join(repoRoot, 'crates', 'vox-orchestrator-mcp', 'src', 'tool_aliases.rs');

function loadAliasesFromRust() {
    const text = fs.readFileSync(rustAliasesPath, 'utf8');
    const map = new Map();
    const re = /\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*\)/g;
    let m;
    while ((m = re.exec(text)) !== null) {
        map.set(m[1], m[2]);
    }
    return map;
}

const ALIASES = loadAliasesFromRust();

function canonicalName(name) {
    return ALIASES.get(name) ?? name;
}

const raw = fs.readFileSync(yamlPath, 'utf8');
const canonical = new Set();
const re = /^\s*-\s*name:\s*(?:"([^"]+)"|([a-zA-Z0-9_:]+))/gm;
let m;
while ((m = re.exec(raw)) !== null) {
    canonical.add(m[1] ?? m[2]);
}

function collectTsFiles(dir, out = []) {
    for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
        const p = path.join(dir, ent.name);
        if (ent.isDirectory()) {
            if (ent.name === 'node_modules' || ent.name === 'out') continue;
            collectTsFiles(p, out);
        } else if (ent.isFile() && ent.name.endsWith('.ts') && !ent.name.endsWith('.d.ts')) {
            out.push(p);
        }
    }
    return out;
}

const srcDirs = [path.join(extRoot, 'src'), path.join(extRoot, 'webview-ui', 'src')];
const toolCallRe = /\bcall\s*\(\s*['"]([a-zA-Z0-9_:]+)['"]/g;
const used = new Set();

function extractCallToolNames(text) {
    const parts = text.split('callTool(');
    for (let i = 1; i < parts.length; i++) {
        const head = parts[i];
        const mm = head.match(/^\s*\{[\s\S]*?name:\s*['"]([a-zA-Z0-9_:]+)['"]/);
        if (mm) used.add(mm[1]);
    }
}

for (const dir of srcDirs) {
    if (!fs.existsSync(dir)) continue;
    for (const file of collectTsFiles(dir)) {
        const text = fs.readFileSync(file, 'utf8');
        let mm;
        while ((mm = toolCallRe.exec(text)) !== null) {
            used.add(mm[1]);
        }
        extractCallToolNames(text);
    }
}

const missing = [];
for (const name of used) {
    if (!name.startsWith('vox_') && !name.includes('::')) continue;
    const c = canonicalName(name);
    if (!canonical.has(c)) {
        missing.push({ used: name, canonical: c });
    }
}

if (missing.length) {
    console.error('MCP tool parity failures (extension calls unknown canonical tools):');
    for (const x of missing) {
        console.error(`  - ${x.used} -> ${x.canonical}`);
    }
    process.exit(1);
}

const hostSrcOnly = path.join(extRoot, 'src');
const directClientCall = /\b(mcp|this\._mcp)\.call\s*\(/;
const badClientCall = [];
if (fs.existsSync(hostSrcOnly)) {
    for (const file of collectTsFiles(hostSrcOnly)) {
        const norm = file.replace(/\\/g, '/');
        if (norm.endsWith('/VoxMcpClient.ts')) continue;
        const text = fs.readFileSync(file, 'utf8');
        if (directClientCall.test(text)) badClientCall.push(norm);
    }
}
if (badClientCall.length) {
    console.error('MCP client calls must go through VoxMcpClient methods only (no mcp.call / this._mcp.call outside VoxMcpClient.ts):');
    for (const f of badClientCall) console.error(`  - ${f}`);
    process.exit(1);
}

console.log(`OK: ${used.size} tool refs + no stray mcp.call in extension host.`);
