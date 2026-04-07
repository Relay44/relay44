'use client';

import { useState, useEffect, useCallback } from 'react';
import { useConfig, useWalletClient } from 'wagmi';
import { readContract, waitForTransactionReceipt, writeContract } from 'wagmi/actions';
import { parseEther, formatEther } from 'viem';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { BASE_CHAIN_ID } from '@/lib/constants';
import {
  RELAY_TOKEN_ADDRESS,
  RELAY_STAKING_ADDRESS,
  ERC20_ABI,
  RELAY_STAKING_ABI,
  assertContractAddress,
} from '@/lib/contracts';

const TIER_NAMES = ['Bronze', 'Silver', 'Gold', 'Diamond'] as const;
const TIER_DISCOUNTS = ['0%', '25%', '50%', '75%'] as const;
const LOCK_PRESETS = [
  { label: '7 days', seconds: 7 * 86400 },
  { label: '30 days', seconds: 30 * 86400 },
  { label: '90 days', seconds: 90 * 86400 },
  { label: '365 days', seconds: 365 * 86400 },
] as const;

interface StakeInfo {
  amount: bigint;
  unlockAt: bigint;
  tier: number;
  pendingReward: bigint;
}

export function StakingPanel() {
  const wallet = useBaseWallet();
  const config = useConfig();
  const { data: walletClient } = useWalletClient();

  const [tab, setTab] = useState<'stake' | 'unstake'>('stake');
  const [amount, setAmount] = useState('');
  const [lockPreset, setLockPreset] = useState(1);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const [relayBalance, setRelayBalance] = useState<bigint>(0n);
  const [stakeInfo, setStakeInfo] = useState<StakeInfo | null>(null);
  const [totalStaked, setTotalStaked] = useState<bigint>(0n);

  const tokenAddr = RELAY_TOKEN_ADDRESS
    ? assertContractAddress(RELAY_TOKEN_ADDRESS, 'NEXT_PUBLIC_RELAY_TOKEN_ADDRESS')
    : null;
  const stakingAddr = RELAY_STAKING_ADDRESS
    ? assertContractAddress(RELAY_STAKING_ADDRESS, 'NEXT_PUBLIC_RELAY_STAKING_ADDRESS')
    : null;

  const fetchData = useCallback(async () => {
    if (!wallet.address || !tokenAddr || !stakingAddr) return;
    const addr = wallet.address as `0x${string}`;

    try {
      const [balance, stakeOf, tier, pending, total] = await Promise.all([
        readContract(config, { address: tokenAddr, abi: ERC20_ABI, functionName: 'balanceOf', args: [addr] }),
        readContract(config, { address: stakingAddr, abi: RELAY_STAKING_ABI, functionName: 'stakeOf', args: [addr] }),
        readContract(config, { address: stakingAddr, abi: RELAY_STAKING_ABI, functionName: 'getTier', args: [addr] }),
        readContract(config, { address: stakingAddr, abi: RELAY_STAKING_ABI, functionName: 'pendingRewardOf', args: [addr] }),
        readContract(config, { address: stakingAddr, abi: RELAY_STAKING_ABI, functionName: 'totalStaked' }),
      ]);

      setRelayBalance(balance as bigint);
      const [stakeAmount, unlockAt] = stakeOf as [bigint, bigint];
      setStakeInfo({
        amount: stakeAmount,
        unlockAt,
        tier: Number(tier),
        pendingReward: pending as bigint,
      });
      setTotalStaked(total as bigint);
    } catch (err) {
      console.error('Failed to fetch staking data:', err);
    }
  }, [wallet.address, tokenAddr, stakingAddr, config]);

  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 15_000);
    return () => clearInterval(interval);
  }, [fetchData]);

  if (!tokenAddr || !stakingAddr) {
    return (
      <div className="p-4 border border-border text-sm text-text-secondary">
        Staking contracts are not configured for this environment.
      </div>
    );
  }

  const handleStake = async () => {
    if (!wallet.isConnected) { await wallet.connect(); return; }
    await wallet.ensureBaseChain();
    if (!walletClient) return;

    const parsed = parseEther(amount);
    if (parsed <= 0n) { setError('Enter a valid amount'); return; }

    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      const addr = wallet.address as `0x${string}`;
      const allowance = await readContract(config, {
        address: tokenAddr, abi: ERC20_ABI, functionName: 'allowance', args: [addr, stakingAddr],
      }) as bigint;

      if (allowance < parsed) {
        const approveTx = await writeContract(config, {
          address: tokenAddr, abi: ERC20_ABI, functionName: 'approve',
          args: [stakingAddr, parsed],
        });
        await waitForTransactionReceipt(config, { hash: approveTx });
      }

      const lockSeconds = BigInt(LOCK_PRESETS[lockPreset].seconds);
      const stakeTx = await writeContract(config, {
        address: stakingAddr, abi: RELAY_STAKING_ABI, functionName: 'stake',
        args: [parsed, lockSeconds],
      });
      await waitForTransactionReceipt(config, { hash: stakeTx });

      setSuccess(`Staked ${amount} RELAY for ${LOCK_PRESETS[lockPreset].label}`);
      setAmount('');
      await fetchData();
    } catch (err: any) {
      setError(err?.shortMessage || err?.message || 'Stake failed');
    } finally {
      setLoading(false);
    }
  };

  const handleUnstake = async () => {
    if (!wallet.isConnected) { await wallet.connect(); return; }
    await wallet.ensureBaseChain();

    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      const tx = await writeContract(config, {
        address: stakingAddr, abi: RELAY_STAKING_ABI, functionName: 'unstake',
      });
      await waitForTransactionReceipt(config, { hash: tx });
      setSuccess('Unstaked successfully');
      await fetchData();
    } catch (err: any) {
      setError(err?.shortMessage || err?.message || 'Unstake failed');
    } finally {
      setLoading(false);
    }
  };

  const handleClaim = async () => {
    if (!wallet.isConnected) { await wallet.connect(); return; }
    await wallet.ensureBaseChain();

    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      const tx = await writeContract(config, {
        address: stakingAddr, abi: RELAY_STAKING_ABI, functionName: 'claimRewards',
      });
      await waitForTransactionReceipt(config, { hash: tx });
      setSuccess('Rewards claimed');
      await fetchData();
    } catch (err: any) {
      setError(err?.shortMessage || err?.message || 'Claim failed');
    } finally {
      setLoading(false);
    }
  };

  const hasStake = stakeInfo && stakeInfo.amount > 0n;
  const isLocked = hasStake && BigInt(Math.floor(Date.now() / 1000)) < stakeInfo.unlockAt;
  const hasPendingRewards = stakeInfo && stakeInfo.pendingReward > 0n;

  return (
    <div className="space-y-6">
      {/* Stats */}
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <Card>
          <CardContent className="p-4">
            <p className="text-sm text-text-secondary">Your Stake</p>
            <p className="text-xl font-semibold text-text-primary">
              {hasStake ? formatEther(stakeInfo.amount) : '0'} RELAY
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <p className="text-sm text-text-secondary">Your Tier</p>
            <p className="text-xl font-semibold text-text-primary">
              {stakeInfo ? TIER_NAMES[stakeInfo.tier] : 'Bronze'}
            </p>
            <p className="text-xs text-text-muted">
              {stakeInfo ? TIER_DISCOUNTS[stakeInfo.tier] : '0%'} fee discount
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <p className="text-sm text-text-secondary">Pending Rewards</p>
            <p className="text-xl font-semibold text-text-primary">
              {hasPendingRewards ? formatEther(stakeInfo.pendingReward) : '0'} RELAY
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <p className="text-sm text-text-secondary">Total Staked</p>
            <p className="text-xl font-semibold text-text-primary">
              {formatEther(totalStaked)} RELAY
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Lock status */}
      {hasStake && (
        <div className="p-3 border border-border text-sm text-text-secondary">
          {isLocked ? (
            <>Locked until {new Date(Number(stakeInfo.unlockAt) * 1000).toLocaleDateString()}</>
          ) : (
            <>Lock period expired — you can unstake</>
          )}
        </div>
      )}

      {/* Tabs */}
      <div className="flex gap-2 border-b border-border">
        <button
          onClick={() => setTab('stake')}
          className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            tab === 'stake'
              ? 'border-text-primary text-text-primary'
              : 'border-transparent text-text-muted hover:text-text-secondary'
          }`}
        >
          Stake
        </button>
        <button
          onClick={() => setTab('unstake')}
          className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            tab === 'unstake'
              ? 'border-text-primary text-text-primary'
              : 'border-transparent text-text-muted hover:text-text-secondary'
          }`}
        >
          Unstake
        </button>
      </div>

      {/* Feedback */}
      {error && (
        <div className="p-3 bg-ask/10 border border-ask/20">
          <p className="text-sm text-ask">{error}</p>
        </div>
      )}
      {success && (
        <div className="p-3 bg-yes/10 border border-yes/20">
          <p className="text-sm text-yes">{success}</p>
        </div>
      )}

      {/* Stake tab */}
      {tab === 'stake' && (
        <Card>
          <CardHeader>
            <CardTitle>Stake RELAY</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {hasStake && (
              <div className="p-3 border border-border text-sm text-text-secondary">
                You already have an active stake. Unstake first to create a new one.
              </div>
            )}

            <div>
              <label className="block text-sm text-text-secondary mb-1">Amount</label>
              <div className="flex gap-2">
                <Input
                  type="number"
                  value={amount}
                  onChange={(e) => setAmount(e.target.value)}
                  placeholder="0.0"
                  min="1"
                  step="0.01"
                  disabled={!!hasStake}
                />
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={!!hasStake}
                  onClick={() => setAmount(formatEther(relayBalance))}
                >
                  Max
                </Button>
              </div>
              <p className="mt-1 text-xs text-text-muted">
                Balance: {formatEther(relayBalance)} RELAY
              </p>
            </div>

            <div>
              <label className="block text-sm text-text-secondary mb-2">Lock Duration</label>
              <div className="grid grid-cols-4 gap-2">
                {LOCK_PRESETS.map((preset, i) => (
                  <button
                    key={preset.seconds}
                    onClick={() => setLockPreset(i)}
                    disabled={!!hasStake}
                    className={`px-3 py-2 text-sm border transition-colors ${
                      lockPreset === i
                        ? 'border-text-primary bg-bg-secondary text-text-primary'
                        : 'border-border text-text-muted hover:text-text-secondary'
                    }`}
                  >
                    {preset.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Tier preview */}
            <div className="p-4 bg-bg-tertiary space-y-2">
              <p className="text-sm font-medium text-text-primary">Tier Benefits</p>
              <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs text-text-secondary">
                <span>Bronze (&lt;1K)</span><span>No discount</span>
                <span>Silver (1K+)</span><span>25% fee discount</span>
                <span>Gold (10K+)</span><span>50% fee discount + API access</span>
                <span>Diamond (100K+)</span><span>75% fee discount + governance</span>
              </div>
            </div>

            <Button
              variant="primary"
              size="lg"
              className="w-full"
              onClick={handleStake}
              loading={loading}
              disabled={!!hasStake || !amount || parseFloat(amount) <= 0}
            >
              {!wallet.isConnected ? 'Connect Wallet' : 'Stake RELAY'}
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Unstake tab */}
      {tab === 'unstake' && (
        <Card>
          <CardHeader>
            <CardTitle>Unstake & Claim</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {!hasStake ? (
              <p className="text-sm text-text-secondary">No active stake to withdraw.</p>
            ) : (
              <>
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <p className="text-sm text-text-secondary">Staked Amount</p>
                    <p className="text-lg font-semibold text-text-primary">
                      {formatEther(stakeInfo.amount)} RELAY
                    </p>
                  </div>
                  <div>
                    <p className="text-sm text-text-secondary">Pending Rewards</p>
                    <p className="text-lg font-semibold text-text-primary">
                      {formatEther(stakeInfo.pendingReward)} RELAY
                    </p>
                  </div>
                </div>

                <div className="flex gap-3">
                  <Button
                    variant="primary"
                    size="lg"
                    className="flex-1"
                    onClick={handleUnstake}
                    loading={loading}
                    disabled={!!isLocked}
                  >
                    {isLocked ? 'Locked' : 'Unstake + Claim'}
                  </Button>
                  <Button
                    variant="secondary"
                    size="lg"
                    className="flex-1"
                    onClick={handleClaim}
                    loading={loading}
                    disabled={!hasPendingRewards}
                  >
                    Claim Rewards Only
                  </Button>
                </div>
              </>
            )}
          </CardContent>
        </Card>
      )}
    </div>
  );
}
