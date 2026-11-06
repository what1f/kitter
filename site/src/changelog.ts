import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Lang } from './i18n';

export interface Release {
  version: string;
  date: string;
  /** Bullet items and paragraphs as inline HTML, in file order. */
  blocks: ({ kind: 'list'; items: string[] } | { kind: 'text'; html: string })[];
  compare?: { label: string; href: string };
}

const files: Record<Lang, string> = { en: 'CHANGELOG.md', zh: 'CHANGELOG.zh-CN.md' };

const escape = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

// Only what the changelog uses: `code` and [links](url).
const inline = (s: string) =>
  escape(s)
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, '<a href="$2" rel="noopener">$1</a>');

/** Reads the changelog from the repo root at build time. */
export function releases(lang: Lang): Release[] {
  const text = readFileSync(resolve(process.cwd(), '..', files[lang]), 'utf8');
  return text
    .split(/^## /m)
    .slice(1)
    .map((section) => {
      const [heading, ...lines] = section.trim().split('\n');
      const [version, date = ''] = heading.split(/\s+—\s+/);
      const release: Release = { version: version.trim(), date: date.trim(), blocks: [] };
      for (const raw of lines) {
        const line = raw.trim();
        if (!line) continue;
        const compare = line.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
        if (compare) {
          release.compare = { label: compare[1], href: compare[2] };
        } else if (line.startsWith('- ')) {
          const last = release.blocks.at(-1);
          const item = inline(line.slice(2));
          if (last?.kind === 'list') last.items.push(item);
          else release.blocks.push({ kind: 'list', items: [item] });
        } else {
          release.blocks.push({ kind: 'text', html: inline(line) });
        }
      }
      return release;
    });
}
