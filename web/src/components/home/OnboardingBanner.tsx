'use client';

import Link from 'next/link';
import { useEffect, useState } from 'react';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { useSessionState } from '@/hooks/useSessionState';

const DISMISSED_KEY = 'relay:onboarding-dismissed';

type Step = 'connect' | 'deposit' | 'trade';

function currentStep(connected: boolean, hasSession: boolean): Step | null {
  if (!connected) return 'connect';
  if (!hasSession) return 'connect';
  return 'deposit';
}

const STEPS: Record<Step, { label: string; description: string; href: string; action: string }> = {
  connect: {
    label: 'Connect wallet',
    description: 'Link your Base wallet to get started with prediction markets.',
    href: '/how-it-works',
    action: 'How it works',
  },
  deposit: {
    label: 'Deposit & trade',
    description: 'Fund your wallet and place your first prediction on any live market.',
    href: '/markets',
    action: 'Browse markets',
  },
  trade: {
    label: 'Start trading',
    description: 'You\'re set up. Explore markets and place your first order.',
    href: '/markets',
    action: 'Browse markets',
  },
};

export function OnboardingBanner() {
  const { isConnected } = useBaseWallet();
  const { hasSession, sessionRestored } = useSessionState();
  const [dismissed, setDismissed] = useState(true);

  useEffect(() => {
    setDismissed(localStorage.getItem(DISMISSED_KEY) === '1');
  }, []);

  if (!sessionRestored || dismissed) return null;

  const step = currentStep(isConnected, hasSession);
  if (!step) return null;

  const info = STEPS[step];

  const dismiss = () => {
    localStorage.setItem(DISMISSED_KEY, '1');
    setDismissed(true);
  };

  return (
    <div className="border-b border-border bg-bg-secondary/60 px-4 py-3 sm:px-6">
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-center gap-4 min-w-0">
          <div className="flex items-center gap-2 shrink-0">
            {(['connect', 'deposit', 'trade'] as Step[]).map((s, i) => (
              <div
                key={s}
                className={`w-2 h-2 rounded-full ${
                  s === step
                    ? 'bg-accent'
                    : i < ['connect', 'deposit', 'trade'].indexOf(step)
                      ? 'bg-accent/40'
                      : 'bg-border'
                }`}
              />
            ))}
          </div>
          <div className="min-w-0">
            <span className="text-[11px] uppercase tracking-[0.14em] text-accent font-mono">
              {info.label}
            </span>
            <span className="text-sm text-text-secondary ml-3 hidden sm:inline">
              {info.description}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <Link
            href={info.href}
            className="inline-flex h-8 items-center border border-accent px-3 text-[0.65rem] uppercase tracking-[0.12em] text-accent transition-colors hover:bg-accent/10"
          >
            {info.action}
          </Link>
          <button
            type="button"
            onClick={dismiss}
            className="inline-flex h-8 w-8 items-center justify-center text-text-muted hover:text-text-primary transition-colors"
            aria-label="Dismiss"
          >
            ×
          </button>
        </div>
      </div>
    </div>
  );
}
