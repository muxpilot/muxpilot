// Single source of truth for the docs section: sidebar order + prev/next links.
export type DocLink = {
  slug: string;
  href: string;
  title: string;
};

export const DOCS: DocLink[] = [
  {
    slug: "introduction",
    href: "/docs/introduction",
    title: "Introduction",
  },
  {
    slug: "installation",
    href: "/docs/installation",
    title: "Installation",
  },
  { slug: "cli", href: "/docs/cli", title: "CLI reference" },
  {
    slug: "tmux-plugin",
    href: "/docs/tmux-plugin",
    title: "tmux plugin",
  },
  {
    slug: "configuration",
    href: "/docs/configuration",
    title: "Configuration",
  },
];

export function docNeighbors(slug: string): {
  prev: DocLink | null;
  next: DocLink | null;
} {
  const i = DOCS.findIndex((d) => d.slug === slug);
  if (i === -1) return { prev: null, next: null };
  return {
    prev: i > 0 ? DOCS[i - 1] : null,
    next: i < DOCS.length - 1 ? DOCS[i + 1] : null,
  };
}
