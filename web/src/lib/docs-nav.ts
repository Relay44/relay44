export interface DocsNavItem {
  title: string;
  href: string;
}

export interface DocsNavGroup {
  title: string;
  items: DocsNavItem[];
}

export type DocsNavEntry = DocsNavItem | DocsNavGroup;

export function isNavGroup(entry: DocsNavEntry): entry is DocsNavGroup {
  return 'items' in entry;
}

export const docsNav: DocsNavEntry[] = [
  { title: 'Overview', href: '/docs' },
  {
    title: 'User Guides',
    items: [
      { title: 'Getting Started', href: '/docs/guides/getting-started' },
      { title: 'Markets', href: '/docs/guides/markets' },
      { title: 'Agents', href: '/docs/guides/agents' },
      { title: 'Decision Cells', href: '/docs/guides/decisions' },
      { title: 'Credentials', href: '/docs/guides/credentials' },
      { title: 'Strategies', href: '/docs/guides/strategies' },
    ],
  },
  {
    title: 'Developers',
    items: [
      { title: 'Overview', href: '/docs/developers' },
      { title: 'Quickstart', href: '/docs/developers/quickstart' },
      { title: 'Authentication', href: '/docs/developers/authentication' },
      { title: 'WebSocket', href: '/docs/developers/websocket' },
    ],
  },
  {
    title: 'API Reference',
    items: [
      { title: 'Overview', href: '/docs/api' },
      { title: 'Auth', href: '/docs/api/auth' },
      { title: 'Markets', href: '/docs/api/markets' },
      { title: 'Orders', href: '/docs/api/orders' },
      { title: 'Positions', href: '/docs/api/positions' },
      { title: 'Agents', href: '/docs/api/agents' },
      { title: 'Decisions', href: '/docs/api/decisions' },
      { title: 'EVM / On-chain', href: '/docs/api/evm' },
      { title: 'WebSocket', href: '/docs/api/websocket' },
    ],
  },
  {
    title: 'Contracts',
    items: [{ title: 'Overview', href: '/docs/contracts' }],
  },
];

/** Flat list of all doc pages for prev/next navigation. */
export function flattenNav(): DocsNavItem[] {
  const flat: DocsNavItem[] = [];
  for (const entry of docsNav) {
    if (isNavGroup(entry)) {
      flat.push(...entry.items);
    } else {
      flat.push(entry);
    }
  }
  return flat;
}
