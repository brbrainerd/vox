/**
 * remark-vox-include — processes {{#include path:anchor}} directives inside
 * fenced code blocks, identical to the mdBook include syntax.
 *
 * Build-time errors are thrown for any unresolved path or anchor so that CI
 * (the Starlight pnpm build step) fails immediately instead of silently
 * emitting the raw directive as visible text.
 *
 * Anchor format in source files:
 *   // ANCHOR: name
 *   ...code...
 *   // ANCHOR_END: name
 *
 * Nested ANCHOR/ANCHOR_END markers inside the extracted region are stripped.
 */

import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

// Matches {{#include relative/path/file.ext:anchor}} or {{#include path}} (no anchor)
const INCLUDE_RE = /^\s*\{\{#include\s+([^}:\s]+?)(?::([^}\s]+))?\}\}\s*$/;

/** Strip ANCHOR/ANCHOR_END marker lines from arbitrary source content. */
const ANCHOR_MARKER_RE = /^\s*\/\/\s*ANCHOR(?:_END)?:/;

/**
 * Extract the region between `// ANCHOR: name` and `// ANCHOR_END: name`,
 * stripping all nested anchor marker lines, then trimming blank edges.
 * @param {string} content  Full file content
 * @param {string} anchor   Anchor name to extract
 * @param {string} srcPath  Source file path (for error messages)
 * @returns {string}
 */
function extractAnchor(content, anchor, srcPath) {
  const startRe = new RegExp(`^\\s*\\/\\/\\s*ANCHOR:\\s*${anchor}\\s*$`);
  const endRe = new RegExp(`^\\s*\\/\\/\\s*ANCHOR_END:\\s*${anchor}\\s*$`);
  const lines = content.split('\n');
  let inside = false;
  const out = [];

  for (const line of lines) {
    if (!inside) {
      if (startRe.test(line)) { inside = true; }
    } else {
      if (endRe.test(line)) break;
      if (!ANCHOR_MARKER_RE.test(line)) out.push(line);
    }
  }

  if (!inside) {
    throw new Error(`Anchor '${anchor}' not found in ${srcPath}`);
  }

  // Trim leading/trailing blank lines
  while (out.length && !out[0].trim()) out.shift();
  while (out.length && !out[out.length - 1].trim()) out.pop();
  return out.join('\n');
}

/**
 * Simple recursive node visitor (avoids adding unist-util-visit as a dep).
 * @param {object} tree
 * @param {string} type
 * @param {(node: object) => void} fn
 */
function visit(tree, type, fn) {
  if (tree.type === type) fn(tree);
  if (Array.isArray(tree.children)) {
    for (const child of tree.children) visit(child, type, fn);
  }
}

/**
 * Remark plugin factory.
 *
 * Usage in astro.config.mjs:
 *   import { remarkVoxInclude } from './src/plugins/remark-vox-include.mjs';
 *   // markdown: { remarkPlugins: [remarkVoxInclude] }
 */
export function remarkVoxInclude() {
  return function transformer(tree, file) {
    // Resolve the directory of the file being processed.
    // `file.path` is the absolute path set by Astro's content pipeline.
    let fileDir;
    if (file.path) {
      // file.path may be a file:// URL on some platforms
      const rawPath = file.path.startsWith('file://')
        ? fileURLToPath(file.path)
        : file.path;
      fileDir = dirname(rawPath);
    } else if (file.history && file.history.length > 0) {
      fileDir = dirname(file.history[0]);
    } else {
      // No path info — skip silently (e.g., virtual content in tests)
      return;
    }

    const errors = [];

    visit(tree, 'code', (node) => {
      const match = node.value.match(INCLUDE_RE);
      if (!match) return;

      const [, relativePath, anchor] = match;
      const absPath = resolve(fileDir, relativePath);

      let fileContent;
      try {
        fileContent = readFileSync(absPath, 'utf8');
      } catch {
        errors.push(`  Cannot read '${relativePath}' (resolved: ${absPath})`);
        return;
      }

      try {
        node.value = anchor
          ? extractAnchor(fileContent, anchor, absPath)
          : fileContent.trimEnd();
      } catch (err) {
        errors.push(`  ${relativePath}:${anchor} — ${err.message}`);
      }
    });

    if (errors.length) {
      const location = file.path || '(unknown file)';
      throw new Error(
        `remark-vox-include: ${errors.length} unresolved include(s) in ${location}:\n${errors.join('\n')}\n` +
        `Fix the paths/anchors or run \`cargo run -p vox-doc-pipeline -- --lint-only\` to validate all includes.`
      );
    }
  };
}
