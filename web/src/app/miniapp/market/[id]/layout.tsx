import type { Metadata } from 'next';
import { buildPageMetadata } from '@/lib/seo';
import { fetchSeoMarket } from '@/lib/server/seo';

interface Props {
  params: Promise<{ id: string }>;
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { id } = await params;
  const marketId = decodeURIComponent(id);
  const market = await fetchSeoMarket(marketId);

  if (!market) {
    return buildPageMetadata({
      title: 'Market',
      description: 'Trade prediction markets in the Relay44 Farcaster miniapp.',
      path: `/miniapp/market/${id}`,
      noIndex: true,
    });
  }

  return buildPageMetadata({
    title: market.question,
    description: market.description || `Trade on "${market.question}" in the Relay44 miniapp.`,
    path: `/miniapp/market/${encodeURIComponent(market.id)}`,
    image: market.imageUrl,
    keywords: [market.category, market.provider, 'farcaster', 'miniapp'].filter(Boolean) as string[],
  });
}

export default function MiniappMarketLayout({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}
