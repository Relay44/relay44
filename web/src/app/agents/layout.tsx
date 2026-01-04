import { StructuredData } from '@/components/seo/StructuredData';
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Agents',
  description: 'Explore autonomous agent lanes, execution surfaces, and external-agent workflows on relay44.',
  path: '/agents',
  keywords: ['agents', 'autonomous trading agents', 'external agents'],
});

export default function AgentsLayout({ children }: { children: React.ReactNode }) {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/agents',
            name: 'relay44 agents',
            description:
              'Explore autonomous agent lanes, execution surfaces, and external-agent workflows on relay44.',
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
