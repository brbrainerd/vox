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

  // Fail the build rather than shipping an empty feed. Both known breakages
  // of this endpoint -- filtering on a `last_updated` key no live doc carries,
  // and a Windows cwd that made `git` unspawnable -- produced a well-formed
  // RSS document with zero items and a green build. Nothing noticed either.
  if (items.length === 0) {
    throw new Error(
      `feed.xml produced 0 items from ${docs.length} docs and ` +
        `${gitDates.size} git dates. Refusing to publish an empty feed.`,
    );
  }

  return rss({
    title: 'Vox: The AI-Native Programming Language — Docs',
    description: 'Official documentation updates for the Vox language.',
    site: context.site!,
    items,
    customData: '<language>en-us</language>',
  });
}
