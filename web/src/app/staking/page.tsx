'use client';

import { PageShell } from '@/components/layout';
import { StakingPanel } from '@/components/staking';

export default function StakingPage() {
  return (
    <PageShell>
      <div className="py-8">
        <div className="mx-auto max-w-3xl">
          <h1 className="text-2xl font-bold text-text-primary mb-2">Stake $RELAY</h1>
          <p className="mb-4 text-sm leading-6 text-text-secondary">
            Lock RELAY tokens to earn staking rewards, unlock tiered fee discounts,
            and gain priority access to platform features.
          </p>
          <p className="mb-8 text-xs text-text-muted">
            See{' '}
            <a
              href="/tokenomics"
              className="text-text-secondary underline-offset-2 hover:text-text-primary hover:underline"
            >
              /tokenomics
            </a>{' '}
            for the full fee flow, reward allocation, and roadmap.
          </p>
          <StakingPanel />
        </div>
      </div>
    </PageShell>
  );
}
