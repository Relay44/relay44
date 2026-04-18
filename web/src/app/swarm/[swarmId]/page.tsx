'use client';

import { use } from 'react';
import { PageShell } from '@/components/layout';
import { SwarmPanel } from '@/components/messaging';

interface Props {
  params: Promise<{ swarmId: string }>;
}

export default function SwarmPage({ params }: Props) {
  const { swarmId } = use(params);

  return (
    <PageShell>
      <div className="py-4" style={{ height: 'calc(100vh - 8rem)' }}>
        <div className="mx-auto max-w-3xl h-full">
          <SwarmPanel swarmId={swarmId} />
        </div>
      </div>
    </PageShell>
  );
}
