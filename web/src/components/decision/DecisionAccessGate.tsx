'use client';

import Link from 'next/link';
import type { ReactNode } from 'react';

import { useAuth, useSessionState } from '@/hooks';
import { Card, Button } from '@/components/ui';

interface DecisionAccessGateProps {
  children: ReactNode;
  title?: string;
  body?: string;
}

export function DecisionAccessGate({
  children,
  title = 'Decision cells are private to your account',
  body = 'Connect your wallet and authenticate before creating, editing, or automating a decision cell.',
}: DecisionAccessGateProps) {
  const { walletConnected, login, isLoading, error } = useAuth();
  const { hasSession, sessionRestored } = useSessionState();

  if (!walletConnected) {
    return (
      <Card className="mx-auto max-w-2xl">
        <h2 className="text-xl font-semibold text-text-primary">Connect your wallet</h2>
        <p className="mt-3 text-sm text-text-secondary">{body}</p>
        <div className="mt-5 flex flex-wrap gap-3">
          <Link
            href="/markets"
            className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
          >
            Browse markets
          </Link>
          <Link
            href="/portfolio"
            className="inline-flex h-10 items-center border border-accent px-4 text-sm uppercase tracking-[0.12em] text-accent transition-colors hover:bg-accent/10"
          >
            Portfolio
          </Link>
        </div>
      </Card>
    );
  }

  if (!sessionRestored) {
    return (
      <Card className="mx-auto max-w-2xl">
        <h2 className="text-xl font-semibold text-text-primary">Restoring session</h2>
        <p className="mt-3 text-sm text-text-secondary">
          Checking your wallet session before loading private decision data.
        </p>
      </Card>
    );
  }

  if (!hasSession) {
    return (
      <Card className="mx-auto max-w-2xl">
        <h2 className="text-xl font-semibold text-text-primary">Authenticate wallet</h2>
        <p className="mt-3 text-sm text-text-secondary">{title}</p>
        <p className="mt-2 text-sm text-text-secondary">
          A SIWE signature is required so the app can scope decision cells, alerts, and automation
          to your wallet.
        </p>
        {error ? <p className="mt-3 text-sm text-ask">{error}</p> : null}
        <div className="mt-5 flex flex-wrap gap-3">
          <Button type="button" onClick={() => void login()} loading={isLoading}>
            Authenticate wallet
          </Button>
          <Link
            href="/settings/credentials"
            className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
          >
            Credentials
          </Link>
        </div>
      </Card>
    );
  }

  return <>{children}</>;
}
