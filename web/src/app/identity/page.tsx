'use client';

import { useCallback, useEffect, useState } from 'react';
import { useWalletClient, useConfig } from 'wagmi';
import { waitForTransactionReceipt } from 'wagmi/actions';

import { PageShell } from '@/components/layout';
import { Button, Card, CardContent, useToast } from '@/components/ui';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { api } from '@/lib/api';

const TIER_LABELS: Record<number, string> = {
  0: 'Unverified',
  1: 'Basic',
  2: 'Verified',
  3: 'Institutional',
};

const TIER_DESCRIPTIONS: Record<number, string> = {
  0: 'Default tier with minimal permissions. Suitable for testing.',
  1: 'Basic agent identity. Enables standard on-chain operations.',
  2: 'Verified agent identity. Unlocked via attestation or governance.',
  3: 'Institutional tier. Reserved for credentialed operators.',
};

interface IdentityInfo {
  wallet: string;
  tier: number;
  active: boolean;
  token_id?: number;
}

export default function IdentityPage() {
  const { address, isConnected, ensureBaseChain } = useBaseWallet();
  const { data: walletClient } = useWalletClient();
  const config = useConfig();
  const { addToast } = useToast();

  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [selectedTier, setSelectedTier] = useState(1);

  const fetchIdentity = useCallback(async () => {
    if (!address) {
      setIdentity(null);
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      const data = await api.getIdentity(address);
      setIdentity(data as IdentityInfo);
    } catch (err: unknown) {
      const status = (err as { status?: number })?.status;
      if (status === 404) {
        setIdentity(null);
      } else {
        addToast('Failed to load identity: ' + (err as Error).message, 'error');
      }
    } finally {
      setLoading(false);
    }
  }, [address, addToast]);

  useEffect(() => {
    fetchIdentity();
  }, [fetchIdentity]);

  const handleRegister = async () => {
    if (!address || !walletClient) return;
    setSubmitting(true);
    try {
      await ensureBaseChain();

      const prepared = await api.prepareBaseRegisterIdentity({
        from: address,
        wallet: address,
        tier: selectedTier,
      });

      const hash = await walletClient.sendTransaction({
        account: address as `0x${string}`,
        to: prepared.to as `0x${string}`,
        data: prepared.data,
        value: BigInt(prepared.value),
      });

      await waitForTransactionReceipt(config, { hash });
      addToast('Identity registered successfully!', 'success');
      await fetchIdentity();
    } catch (err: unknown) {
      addToast('Transaction failed: ' + (err as Error).message, 'error');
    } finally {
      setSubmitting(false);
    }
  };

  const handleUpgradeTier = async (newTier: number) => {
    if (!address || !walletClient) return;
    setSubmitting(true);
    try {
      await ensureBaseChain();

      const prepared = await api.prepareBaseSetIdentityTier({
        from: address,
        wallet: address,
        tier: newTier,
      });

      const hash = await walletClient.sendTransaction({
        account: address as `0x${string}`,
        to: prepared.to as `0x${string}`,
        data: prepared.data,
        value: BigInt(prepared.value),
      });

      await waitForTransactionReceipt(config, { hash });
      addToast('Tier updated successfully!', 'success');
      await fetchIdentity();
    } catch (err: unknown) {
      addToast('Tier update failed: ' + (err as Error).message, 'error');
    } finally {
      setSubmitting(false);
    }
  };

  const handleToggleActive = async () => {
    if (!address || !walletClient || !identity) return;
    setSubmitting(true);
    try {
      await ensureBaseChain();

      const prepared = await api.prepareBaseSetIdentityActive({
        from: address,
        wallet: address,
        active: !identity.active,
      });

      const hash = await walletClient.sendTransaction({
        account: address as `0x${string}`,
        to: prepared.to as `0x${string}`,
        data: prepared.data,
        value: BigInt(prepared.value),
      });

      await waitForTransactionReceipt(config, { hash });
      addToast(
        identity.active ? 'Identity deactivated.' : 'Identity activated!',
        'success',
      );
      await fetchIdentity();
    } catch (err: unknown) {
      addToast('Toggle failed: ' + (err as Error).message, 'error');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <PageShell>
      <div className="py-8">
        <div className="mx-auto max-w-2xl space-y-8">
        <div>
          <h1 className="text-2xl font-bold">Agent Identity</h1>
          <p className="text-sm text-text-secondary mt-1">
            Register and manage your on-chain ERC-8004 soulbound identity on
            Base.
          </p>
        </div>

        {!isConnected ? (
          <Card>
            <CardContent className="py-6">
              <p className="text-sm text-text-secondary">
                Connect your wallet to view or register your agent identity.
              </p>
            </CardContent>
          </Card>
        ) : loading ? (
          <div className="animate-pulse text-text-secondary">Loading...</div>
        ) : identity ? (
          /* ── Existing Identity ── */
          <div className="space-y-6">
            <Card>
              <CardContent className="py-6 space-y-4">
                <h2 className="text-lg font-semibold">Your Identity</h2>

                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-text-secondary">Wallet</span>
                    <span className="font-mono text-sm">{identity.wallet}</span>
                  </div>

                  <div className="flex items-center justify-between">
                    <span className="text-sm text-text-secondary">Tier</span>
                    <span className="inline-flex items-center rounded bg-bg-tertiary px-2 py-0.5 text-xs font-medium">
                      {TIER_LABELS[identity.tier] ?? `Tier ${identity.tier}`}
                    </span>
                  </div>

                  <div className="flex items-center justify-between">
                    <span className="text-sm text-text-secondary">Status</span>
                    <span
                      className={
                        identity.active
                          ? 'text-bid text-sm font-medium'
                          : 'text-ask text-sm font-medium'
                      }
                    >
                      {identity.active ? 'Active' : 'Inactive'}
                    </span>
                  </div>

                  {identity.token_id !== undefined && (
                    <div className="flex items-center justify-between">
                      <span className="text-sm text-text-secondary">
                        Token ID
                      </span>
                      <span className="font-mono text-sm">
                        #{identity.token_id}
                      </span>
                    </div>
                  )}
                </div>
              </CardContent>
            </Card>

            {/* ── Upgrade Tier ── */}
            <Card>
              <CardContent className="py-6 space-y-4">
                <h2 className="text-lg font-semibold">Upgrade Tier</h2>
                <p className="text-sm text-text-secondary">
                  Select a new tier for your identity. Higher tiers require
                  on-chain verification or governance approval.
                </p>

                <div className="grid grid-cols-2 gap-3">
                  {[0, 1, 2, 3].map((tier) => (
                    <button
                      key={tier}
                      disabled={tier === identity.tier || submitting}
                      onClick={() => handleUpgradeTier(tier)}
                      className={`rounded border p-3 text-left transition-colors ${
                        tier === identity.tier
                          ? 'border-border-active bg-bg-tertiary opacity-60 cursor-default'
                          : 'border-border-primary hover:border-border-active hover:bg-bg-secondary cursor-pointer'
                      }`}
                    >
                      <span className="block text-sm font-medium">
                        {TIER_LABELS[tier]}
                      </span>
                      <span className="block mt-0.5 text-xs text-text-secondary">
                        {TIER_DESCRIPTIONS[tier]}
                      </span>
                    </button>
                  ))}
                </div>
              </CardContent>
            </Card>

            {/* ── Toggle Active ── */}
            <Card>
              <CardContent className="py-6 space-y-4">
                <h2 className="text-lg font-semibold">Toggle Status</h2>
                <p className="text-sm text-text-secondary">
                  {identity.active
                    ? 'Deactivating your identity will pause all agent operations linked to this wallet.'
                    : 'Reactivate your identity to resume agent operations.'}
                </p>
                <Button
                  variant="primary"
                  disabled={submitting}
                  onClick={handleToggleActive}
                >
                  {submitting
                    ? 'Processing...'
                    : identity.active
                      ? 'Deactivate Identity'
                      : 'Activate Identity'}
                </Button>
              </CardContent>
            </Card>
          </div>
        ) : (
          /* ── Registration Form ── */
          <div className="space-y-6">
            <Card>
              <CardContent className="py-6 space-y-4">
                <h2 className="text-lg font-semibold">
                  What is ERC-8004 Identity?
                </h2>
                <p className="text-sm text-text-secondary leading-relaxed">
                  ERC-8004 is a soulbound NFT standard for on-chain agent
                  identity on Base. Registering an identity binds a
                  non-transferable token to your wallet, enabling verifiable
                  agent operations, reputation tracking, and tiered access across
                  the Relay44 protocol.
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="py-6 space-y-5">
                <h2 className="text-lg font-semibold">Register Identity</h2>

                <div className="space-y-2">
                  <label className="block text-sm font-medium">
                    Select Tier
                  </label>
                  <div className="grid grid-cols-2 gap-3">
                    {[0, 1, 2, 3].map((tier) => (
                      <button
                        key={tier}
                        onClick={() => setSelectedTier(tier)}
                        className={`rounded border p-3 text-left transition-colors cursor-pointer ${
                          selectedTier === tier
                            ? 'border-border-active bg-bg-tertiary'
                            : 'border-border-primary hover:border-border-active hover:bg-bg-secondary'
                        }`}
                      >
                        <span className="block text-sm font-medium">
                          {TIER_LABELS[tier]}
                        </span>
                        <span className="block mt-0.5 text-xs text-text-secondary">
                          {TIER_DESCRIPTIONS[tier]}
                        </span>
                      </button>
                    ))}
                  </div>
                </div>

                <div className="flex items-center gap-3 pt-2">
                  <Button
                    variant="primary"
                    disabled={submitting}
                    onClick={handleRegister}
                  >
                    {submitting ? 'Registering...' : 'Register Identity'}
                  </Button>
                  <span className="inline-flex items-center rounded bg-bg-tertiary px-2 py-0.5 text-xs">
                    {TIER_LABELS[selectedTier]}
                  </span>
                </div>
              </CardContent>
            </Card>
          </div>
        )}
        </div>
      </div>
    </PageShell>
  );
}
