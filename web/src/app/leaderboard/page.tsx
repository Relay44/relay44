import { PageShell } from '@/components/layout';
import { LeaderboardTable } from '@/components/leaderboard';
import { StructuredData } from '@/components/seo/StructuredData';
import {
  buildBreadcrumbStructuredData,
  buildLeaderboardStructuredData,
  buildPageMetadata,
} from '@/lib/seo';
import { fetchSeoLeaderboard } from '@/lib/server/seo';

export const metadata = buildPageMetadata({
  title: 'Leaderboard',
  description: 'Track top traders, performance rankings, and public profiles on relay44.',
  path: '/leaderboard',
  keywords: ['leaderboard', 'top traders', 'trading performance'],
});

export default async function LeaderboardPage() {
  const entries = await fetchSeoLeaderboard(25);

  return (
    <PageShell>
      <StructuredData
        data={[
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Leaderboard', url: '/leaderboard' },
          ]),
          buildLeaderboardStructuredData(entries),
        ]}
      />
      <div className="mx-auto max-w-5xl py-2 sm:py-4">
        <h1 className="mb-6 text-2xl font-bold text-text-primary">Leaderboard</h1>
        <LeaderboardTable limit={100} />
      </div>
    </PageShell>
  );
}
