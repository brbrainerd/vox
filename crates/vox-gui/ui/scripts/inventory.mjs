// crates/vox-gui/ui/scripts/inventory.mjs
import { readdirSync, readFileSync, writeFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = 'src/components/surfaces';
const TOAST = /pushToast\(/;
const HANDLER = /on(Click|Submit|Change|Press)=\{/;

function walk(dir) {
  return readdirSync(dir).flatMap(name => {
    const p = join(dir, name);
    return statSync(p).isDirectory() ? walk(p)
      : p.endsWith('.tsx') && !p.endsWith('.test.tsx') ? [p] : [];
  });
}

const rows = [];
for (const file of walk(ROOT)) {
  const surface = file.split(/[/\\]/)[3] ?? '?';
  readFileSync(file, 'utf8').split('\n').forEach((raw, i) => {
    if (TOAST.test(raw)) rows.push({ surface, file, line: i + 1, kind: 'toast', snippet: raw.trim() });
    if (HANDLER.test(raw)) rows.push({ surface, file, line: i + 1, kind: 'handler', snippet: raw.trim() });
  });
}
writeFileSync('../../../docs/agents/gui-honesty-manifest.json', JSON.stringify(rows, null, 2));
console.log(`manifest: ${rows.length} sites across surfaces`);
