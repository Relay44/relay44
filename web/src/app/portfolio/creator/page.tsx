import { Suspense } from 'react';

import { CreatorDashboardPage } from '@/components/portfolio/CreatorDashboardPage';
import { Card } from '@/components/ui';

function CreatorDashboardFallback() {
  return (
    <section className="flex min-h-[60vh] items-center justify-center">
      <Card className="max-w-xl text-center">
        <h1 className="text-2xl font-semibold text-text-primary">
          Creator dashboard
        </h1>
        <p className="mt-3 text-text-secondary">
          Loading private creator economics.
        </p>
      </Card>
    </section>
  );
}

export default function CreatorPortfolioPage() {
  return (
    <Suspense fallback={<CreatorDashboardFallback />}>
      <CreatorDashboardPage />
    </Suspense>
  );
}
