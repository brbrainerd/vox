import { execFileSync } from 'node:child_process';

/**
 * Last-commit date for every doc under `docs/src`, keyed the way Starlight
 * keys `doc.id`: path relative to the content root, without extension.
 *
 * The RSS feed previously filtered on a `last_updated` frontmatter key. No
 * live doc carries one -- `vox-doc-pipeline` makes hand-authoring it a hard
 * error, because the date is supposed to come from Git. So the filter matched
 * nothing and the feed shipped empty. Git is the source it was always meant
 * to read; this reads it directly, in one subprocess rather than one per file.
 *
 * The repo root is resolved with `git rev-parse --show-toplevel` rather than
 * from `import.meta.url`. At build time this module is bundled into
 * `dist/.prerender/chunks/`, so a path computed relative to the module
 * resolves to `docs-astro/`, not the repo root -- and `git log -- docs/src`
 * from there matches nothing and exits 0, yielding an empty feed with no
 * error. Asking git is correct from any working directory inside the repo.
 */
export function getGitDates(repoRoot = gitRoot()) {
  const out = execFileSync(
    'git',
    ['log', '--format=C|%cI', '--name-only', '--', 'docs/src'],
    { cwd: repoRoot, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
  );

  const dates = new Map();
  let current = null;
  for (const line of out.split('\n')) {
    if (line.startsWith('C|')) {
      current = line.slice(2).trim();
    } else if (line.startsWith('docs/src/') && current) {
      // git log is newest-first, so the first sighting of a path is its
      // most recent commit. Later (older) sightings must not overwrite it.
      const id = line.slice('docs/src/'.length).replace(/\.mdx?$/, '');
      if (!dates.has(id)) dates.set(id, current);
    }
  }
  return dates;
}

function gitRoot() {
  return execFileSync('git', ['rev-parse', '--show-toplevel'], {
    encoding: 'utf8',
  }).trim();
}
