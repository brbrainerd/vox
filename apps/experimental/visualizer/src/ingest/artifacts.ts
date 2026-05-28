/**
 * Best-effort parsers for Vox web artifacts (no TypeScript compiler in-browser).
 */

/** Route `path:` string literals inside `routes.manifest.ts` bodies. */
export function extractRoutePathsFromManifest(src: string): string[] {
  const out: string[] = [];
  const re = /path:\s*"([^"\\]*)"/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src)) !== null) {
    out.push(m[1]);
  }
  return out;
}

/** Exported async function names from `vox-client.ts` (RPC surface). */
export function extractVoxClientEndpoints(src: string): string[] {
  const out: string[] = [];
  const re = /export\s+async\s+function\s+(\w+)\s*\(/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src)) !== null) {
    out.push(m[1]);
  }
  return out;
}
