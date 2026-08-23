import rss from '@astrojs/rss';
import { getCollection } from 'astro:content';
import type { APIContext } from 'astro';
import { getGitDates } from '../utils/git-dates.mjs';

export async function GET(context: APIContext) {
  const docs = await getCollection('docs');
  const gitDates = getGitDates();

  // Dates come from Git, not frontmatter. `last_updated:` is a hard lint
  // error in authored docs (documentation-governance.md), so filtering on it
  // matched zero documents and the feed shipped empty.
  const items = docs
    .map(doc => ({ doc, date: gitDates.get(doc.id) }))
    .filter((entry): entry is { doc: typeof entry.doc; date: string } => Boolean(entry.date))
    .sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime())
    .slice(0, 30)
    .map(({ doc, date }) => ({
      title: doc.data.title,
      pubDate: new Date(date),
      link: `/${doc.id}/`,
      description: doc.data.description ?? '',
    }));

  return rss({
    title: 'Vox: The AI-Native Programming Language — Docs',
    description: 'Official documentation updates for the Vox language.',
    site: context.site!,
    items,
    customData: '<language>en-us</language>',
  });
}
