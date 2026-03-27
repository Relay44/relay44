import { StructuredData } from '@/components/seo/StructuredData';
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Agents',
  description: 'Launch, monitor, and manage market agents across onchain and external venues on Relay44.',
  path: '/agents',
  keywords: ['agents', 'trading agents', 'external agents'],
});

export default function AgentsLayout({ children }: { children: React.ReactNode }) {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/agents',
            name: 'Relay44 agents',
            description: 'Launch, monitor, and manage market agents across onchain and external venues on Relay44.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Agents', url: '/agents' },
          ]),
        ]}
      />
      {children}
    </>
  );
}
