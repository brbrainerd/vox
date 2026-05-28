// Loads vox.tmLanguage.json from the vox-vscode extension into Shiki.
// Path is relative to the docs-astro project root.
// SSOT: apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
// Walk up from docs-astro/src/plugins/ to repo root, then into vox-vscode.
const grammarPath = resolve(
  __dirname,
  '../../../apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json'
);

export const voxGrammar = JSON.parse(readFileSync(grammarPath, 'utf-8'));
voxGrammar.name = 'vox';
