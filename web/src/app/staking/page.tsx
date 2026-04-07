'use client';

import { PageShell } from '@/components/layout';
import { StakingPanel } from '@/components/staking';

export default function StakingPage() {
  return (
    <PageShell>
      <div className="container mx-auto max-w-4xl px-4 py-8">
        <h1 className="text-2xl font-bold text-text-primary mb-2">Stake $RELAY</h1>
        <p className="mb-8 max-w-2xl text-sm leading-6 text-text-secondary">
          Lock RELAY tokens to earn staking rewards, unlock tiered fee discounts,
          and gain priority access to platform features.
        </p>
        <StakingPanel />
      </div>
    </PageShell>
  );
}
