import type { Metadata } from 'next';
import { PageShell } from '@/components/layout';
import { FeatureNotice } from '@/components/runtime/FeatureNotice';
import { buildPageMetadata } from '@/lib/seo';
import { fetchSeoProfile } from '@/lib/server/seo';

interface ProfilePageProps {
  params: Promise<{ wallet: string }>;
}

function isSupportedWallet(wallet: string): boolean {
  const trimmed = wallet.trim();
  const isEvm =
    trimmed.length == 42 &&
    trimmed.startsWith('0x') &&
    [...trimmed.slice(2)].every((value) => /[0-9a-fA-F]/.test(value));
  const isBase58 = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(trimmed);
  return isEvm || isBase58;
}

export async function generateMetadata({ params }: ProfilePageProps): Promise<Metadata> {
  const { wallet } = await params;
  const profile = await fetchSeoProfile(wallet);
  const label = profile?.username || `${wallet.slice(0, 6)}...${wallet.slice(-4)}`;

  return buildPageMetadata({
    title: `${label} profile`,
    description:
      profile?.bio || `View public trading performance, positions, and activity for ${label} on relay44.`,
    path: `/profile/${wallet}`,
    image: profile?.avatarUrl,
    keywords: ['trader profile', 'public profile', label],
    noIndex: true,
    openGraphType: 'profile',
  });
}

export default async function ProfilePage({ params }: ProfilePageProps) {
  const { wallet } = await params;
  const validWallet = isSupportedWallet(wallet);

  return (
    <PageShell>
      <div className="mx-auto max-w-3xl py-2 sm:py-4">
        <FeatureNotice
          title={validWallet ? 'Public profiles are not live yet' : 'Invalid wallet address'}
          body={
            validWallet
              ? 'Profile stats, open positions, and recent activity are still gated for launch. Markets, wallet setup, and agent surfaces remain live.'
              : 'Use a valid wallet address to open a public profile.'
          }
          actionHref={validWallet ? '/markets' : '/leaderboard'}
          actionLabel={validWallet ? 'Browse markets' : 'Open leaderboard'}
        />
      </div>
    </PageShell>
  );
}
