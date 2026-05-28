import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import matter from 'gray-matter';

// SSOT: contracts/documentation/docs-sidebar-section-order.v1.json
const __sidebarUtilsFile = fileURLToPath(import.meta.url);
const __repoRootSidebarOrder = join(__sidebarUtilsFile, '..', '..', '..', '..');
const __sidebarOrderPath = join(
  __repoRootSidebarOrder,
  'contracts/documentation/docs-sidebar-section-order.v1.json'
);
const __sidebarOrder = JSON.parse(readFileSync(__sidebarOrderPath, 'utf8'));
const SECTION_ORDER = __sidebarOrder.sections;
const COLLAPSED_SECTIONS = new Set(__sidebarOrder.collapsed_sections ?? []);

// Status values that earn a sidebar badge so users know a page is not yet stable.
const STATUS_BADGE = {
  experimental: { text: 'Experimental', variant: 'caution' },
  research:     { text: 'Research',     variant: 'note'    },
  roadmap:      { text: 'Roadmap',      variant: 'note'    },
  deprecated:   { text: 'Deprecated',   variant: 'danger'  },
  legacy:       { text: 'Legacy',       variant: 'tip'     },
};

// Directories under docs/src/ that should never appear in the sidebar
const EXCLUDED_DIRS = new Set(['archive', '.well-known']);

function collectPages(dir, root) {
  const pages = [];
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return pages;
  }
  for (const entry of entries) {
    const full = join(dir, entry);
    let stat;
    try {
      stat = statSync(full);
    } catch {
      continue;
    }
    if (stat.isDirectory()) {
      if (EXCLUDED_DIRS.has(entry)) continue;
      pages.push(...collectPages(full, root));
    } else if (extname(entry) === '.md') {
      try {
        const raw = readFileSync(full, 'utf8');
        const { data } = matter(raw);
        // Strip docs/src/ prefix and .md for Starlight link; normalise Windows separators
        const rel = relative(root, full).replace(/\\/g, '/').replace(/\.md$/, '');
        pages.push({
          title:      data.title || entry.replace('.md', ''),
          link:       rel,
          category:   data.category || null,
          sort_order: data.sort_order ?? 999,
          status:     data.status || 'current',
        });
      } catch {
        // skip unreadable / non-frontmatter files
      }
    }
  }
  return pages;
}

function makeItem(p) {
  const badge = STATUS_BADGE[p.status];
  return badge
    ? { label: p.title, link: p.link, badge }
    : { label: p.title, link: p.link };
}

export function getSidebar() {
  // thisFile = <repo>/docs-astro/src/utils/sidebar.mjs
  // go up: utils → src → docs-astro → repo-root
  const thisFile = fileURLToPath(import.meta.url);
  const repoRoot = join(thisFile, '..', '..', '..', '..');
  const docsSrc = join(repoRoot, 'docs', 'src');

  const pages = collectPages(docsSrc, docsSrc);

  const grouped = new Map();
  const rootItems = [];

  for (const page of pages) {
    if (!page.category) {
      rootItems.push(page);
    } else {
      if (!grouped.has(page.category)) grouped.set(page.category, []);
      grouped.get(page.category).push(page);
    }
  }

  const sortFn = (a, b) =>
    a.sort_order - b.sort_order || a.title.localeCompare(b.title);

  const sidebar = [];

  rootItems.sort(sortFn);
  for (const p of rootItems) {
    sidebar.push(makeItem(p));
  }

  for (const section of SECTION_ORDER) {
    const items = grouped.get(section);
    if (!items || items.length === 0) continue;
    items.sort(sortFn);
    sidebar.push({
      label: section,
      items: items.map(makeItem),
      collapsed: COLLAPSED_SECTIONS.has(section),
    });
    grouped.delete(section);
  }

  // Any category not in SECTION_ORDER appended at end (collapsed — catch-all for new sections)
  for (const [section, items] of grouped) {
    items.sort(sortFn);
    sidebar.push({
      label: section,
      items: items.map(makeItem),
      collapsed: true,
    });
  }

  return sidebar;
}
