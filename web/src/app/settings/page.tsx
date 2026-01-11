'use client';

import Link from 'next/link';
import { useEffect, useState } from 'react';

import { useBaseWallet } from '@/hooks/useBaseWallet';
import { useSolanaWallet } from '@/hooks/useSolanaWallet';
import { api, type BaseTokenState } from '@/lib/api';
import { PageShell } from '@/components/layout';
import { Card, Button } from '@/components/ui';
import {
  BASE_RPC_ENDPOINT,
  SOLANA_RPC_ENDPOINT,
} from '@/lib/constants';
import { truncateAddress } from '@/lib/utils';

function formatTokenSupply(totalSupplyHex: string, decimals: number): string {
  if (!totalSupplyHex.startsWith('0x')) return totalSupplyHex;

  const raw = BigInt(totalSupplyHex);
  let divisor = BigInt(1);
  for (let i = 0; i < decimals; i += 1) {
    divisor *= BigInt(10);
  }
  const whole = raw / divisor;
  const fraction = raw % divisor;

  if (fraction === BigInt(0)) {
    return whole.toString();
  }

  const paddedFraction = fraction.toString().padStart(decimals, '0').replace(/0+$/, '');
  return `${whole.toString()}.${paddedFraction}`;
}

export default function SettingsPage() {
  const baseWallet = useBaseWallet();
  const solanaWallet = useSolanaWallet();

  const [baseTokenState, setBaseTokenState] = useState<BaseTokenState | null>(null);
  const [baseTokenError, setBaseTokenError] = useState<string | null>(null);

  const connected = baseWallet.isConnected || solanaWallet.isConnected;
  const walletAddress = baseWallet.address ?? solanaWallet.address;

  useEffect(() => {
    let mounted = true;

    api
      .getBaseTokenState()
      .then((state) => {
        if (!mounted) return;
        setBaseTokenState(state);
        setBaseTokenError(null);
      })
      .catch((error) => {
        if (!mounted) return;
        const message = error instanceof Error ? error.message : 'Unable to load Base token state';
        setBaseTokenError(message);
      });

    return () => {
      mounted = false;
    };
  }, []);

  const disconnectWallet = () => {
    if (baseWallet.isConnected) {
      baseWallet.disconnect();
    }
    if (solanaWallet.isConnected) {
      solanaWallet.disconnect().catch(() => {
      });
    }
  };

  return (
    <PageShell>
      <h1 className="text-2xl font-bold mb-6">Settings</h1>

      {connected && walletAddress && (
        <Card className="mb-6">
          <h2 className="font-semibold mb-4">Wallet</h2>
          <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <div className="text-text-secondary text-sm">Connected Address</div>
              <div className="font-mono break-all">{truncateAddress(walletAddress)}</div>
              <div className="text-text-secondary text-xs mt-1">
                {baseWallet.isConnected ? 'Base' : 'Solana'}
              </div>
            </div>
            <Button variant="danger" size="sm" onClick={disconnectWallet} className="w-full sm:w-auto">
              Disconnect
            </Button>
          </div>
        </Card>
      )}

      <Card className="mb-6">
        <h2 className="font-semibold mb-4">Preferences</h2>
        <div className="space-y-4">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <div className="font-medium">Dark Mode</div>
              <div className="text-text-secondary text-sm">Always enabled</div>
            </div>
            <div className="relative h-6 w-12 self-start bg-accent sm:self-auto">
              <div className="absolute right-1 top-1 w-4 h-4 bg-white " />
            </div>
          </div>
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <div className="font-medium">Push Notifications</div>
              <div className="text-text-secondary text-sm">Price alerts and updates</div>
            </div>
            <div className="relative h-6 w-12 self-start bg-bg-tertiary sm:self-auto">
              <div className="absolute left-1 top-1 w-4 h-4 bg-text-muted " />
            </div>
          </div>
        </div>
      </Card>

      <Card className="mb-6">
        <h2 className="font-semibold mb-4">Network</h2>
        <div className="space-y-2">
          <div className="flex items-center justify-between gap-3 py-2">
            <span>Base</span>
            <span className="w-2 h-2 bg-accent " />
          </div>
          <div className="text-text-secondary text-sm break-all">
            RPC: {BASE_RPC_ENDPOINT}
          </div>
          <div className="text-text-secondary text-sm break-all">
            Solana RPC: {SOLANA_RPC_ENDPOINT}
          </div>
          {baseTokenState && (
            <>
              <div className="text-text-secondary text-sm break-all">
                Token: {baseTokenState.token_address}
              </div>
              <div className="text-text-secondary text-sm">
                Supply: {formatTokenSupply(baseTokenState.total_supply_hex, baseTokenState.decimals)}
              </div>
            </>
          )}
          {baseTokenError && (
            <div className="break-words text-sm text-red-400">
              Token state unavailable: {baseTokenError}
            </div>
          )}
        </div>
      </Card>

      <Card>
        <h2 className="font-semibold mb-4">About</h2>
        <div className="space-y-3 text-sm">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="text-text-secondary">External Credentials</span>
            <Link
              href="/settings/credentials"
              className="border border-border px-3 py-1 text-xs uppercase tracking-[0.12em] text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
            >
              Open vault
            </Link>
          </div>
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="text-text-secondary">Version</span>
            <span>1.0.0</span>
          </div>
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="text-text-secondary">Build</span>
            <span>dev</span>
          </div>
        </div>
      </Card>
    </PageShell>
  );
}
