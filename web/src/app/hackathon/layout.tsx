import { StructuredData } from '@/components/seo/StructuredData';
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Hackathon',
  description: 'Compete in AI agent trading hackathons on Relay44. Build agents, trade on real markets, and win prizes.',
  path: '/hackathon',
  image: '/hackathon-sharing.jpg',
  keywords: ['hackathon', 'ai agents', 'trading competition', 'prediction markets'],
});

export default function HackathonLayout({ children }: { children: React.ReactNode }) {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/hackathon',
            name: 'Relay44 hackathons',
            description: 'Compete in AI agent trading hackathons on Relay44.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Hackathon', url: '/hackathon' },
          ]),
        ]}
      />
      {children}
    </>
  );
}
