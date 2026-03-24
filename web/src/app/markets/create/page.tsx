import { PageShell } from '@/components/layout';
import { CreateMarketForm } from '@/components/market';
import { buildPageMetadata } from '@/lib/seo';
import { getHomeLiveFeed } from '@/lib/server/homeLive';

export const metadata = buildPageMetadata({
  title: 'Draft market',
  description: 'Review and refine a drafted market before publishing on relay44.',
  path: '/markets/create',
  noIndex: true,
});

interface CreateMarketPageProps {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}

function firstParam(value: string | string[] | undefined): string {
  if (Array.isArray(value)) {
    return value[0] || '';
  }
  return value || '';
}

export default async function CreateMarketPage({
  searchParams,
}: CreateMarketPageProps) {
  const params = searchParams ? await searchParams : {};
  const storyId = firstParam(params.story);
  const draftId = firstParam(params.draft);
  const liveFeed = storyId ? await getHomeLiveFeed() : null;
  const draftSlide = liveFeed?.news.find((slide) => slide.id === storyId) ?? null;

  return (
    <PageShell>
      <div className="container mx-auto max-w-2xl px-4 py-8">
        <CreateMarketForm
          draftSlide={draftSlide}
          initialDraftId={draftId}
          initialQuestion={firstParam(params.question)}
          initialDescription={firstParam(params.description)}
          initialCategory={firstParam(params.category)}
          initialResolutionSource={firstParam(params.resolutionSource)}
          initialCustomSource={firstParam(params.customSource)}
          initialTradingEnd={firstParam(params.tradingEnd)}
        />
      </div>
    </PageShell>
  );
}
