'use client';

import { useCallback, useEffect, useState } from 'react';
import { useWalletClient, useConfig } from 'wagmi';
import { waitForTransactionReceipt } from 'wagmi/actions';

import { Button, Card, CardContent, Input, useToast } from '@/components/ui';
import { ReputationBadge } from '@/components/ui/ReputationBadge';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { api } from '@/lib/api';

interface ReputationPanelProps {
  wallet: string;
}

interface Reputation {
  scoreBps?: number;
  confidenceBps?: number;
  events?: number;
  notionalMicrousdc?: string;
}

interface FeedbackEntry {
  client: string;
  feedbackIndex: number;
  value: string;
  valueDecimals: number;
  tag1: string;
  tag2: string;
  revoked: boolean;
}

const ZERO_BYTES32 =
  '0x0000000000000000000000000000000000000000000000000000000000000000';

// int128 bounds: [-(2^127), 2^127 - 1]
const INT128_MAX = BigInt('170141183460469231731687303715884105727');
const INT128_MIN = BigInt('-170141183460469231731687303715884105728');
const MAX_URI_LENGTH = 1024;

function formatUsdc(microusdc?: string): string {
  if (!microusdc) return '—';
  const n = Number(microusdc);
  if (!Number.isFinite(n)) return microusdc;
  return `$${(n / 1_000_000).toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}

function formatAddress(address: string): string {
  if (address.length < 10) return address;
  return `${address.slice(0, 6)}…${address.slice(-4)}`;
}

function formatFeedbackValue(entry: FeedbackEntry): string {
  const raw = Number(entry.value);
  if (!Number.isFinite(raw)) return entry.value;
  if (entry.valueDecimals === 0) return raw.toString();
  return (raw / 10 ** entry.valueDecimals).toFixed(entry.valueDecimals);
}

export function ReputationPanel({ wallet }: ReputationPanelProps) {
  const { address, isConnected, ensureBaseChain } = useBaseWallet();
  const { data: walletClient } = useWalletClient();
  const config = useConfig();
  const { addToast } = useToast();

  const [reputation, setReputation] = useState<Reputation | null>(null);
  const [agentId, setAgentId] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<FeedbackEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [showForm, setShowForm] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  const [valueInput, setValueInput] = useState('80');
  const [feedbackUri, setFeedbackUri] = useState('');

  const fetchData = useCallback(
    async (opts: { silent?: boolean } = {}) => {
      if (!wallet) return;
      if (opts.silent) {
        setRefreshing(true);
      } else {
        setLoading(true);
      }
      try {
        const [rep, fb] = await Promise.all([
          api.getBaseReputation(wallet).catch(() => null),
          api.getBaseReputationFeedback(wallet).catch(() => null),
        ]);
        if (rep) {
          setReputation({
            scoreBps: rep.scoreBps,
            confidenceBps: rep.confidenceBps,
            events: rep.events,
            notionalMicrousdc: rep.notionalMicrousdc,
          });
        } else {
          setReputation(null);
        }
        if (fb) {
          setAgentId(fb.agentId ?? null);
          setFeedback(fb.feedback ?? []);
        }
      } catch (err) {
        addToast(
          'Failed to load reputation: ' + (err as Error).message,
          'error',
        );
      } finally {
        setLoading(false);
        setRefreshing(false);
      }
    },
    [wallet, addToast],
  );

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  const canGiveFeedback =
    isConnected &&
    address != null &&
    agentId != null &&
    address.toLowerCase() !== wallet.toLowerCase();

  const validateForm = (): { ok: true; value: bigint } | { ok: false; error: string } => {
    const raw = valueInput.trim();
    if (raw === '' || raw === '-') {
      return { ok: false, error: 'Value is required' };
    }
    if (!/^-?\d+$/.test(raw)) {
      return { ok: false, error: 'Value must be an integer' };
    }
    let parsed: bigint;
    try {
      parsed = BigInt(raw);
    } catch {
      return { ok: false, error: 'Value must be an integer' };
    }
    if (parsed < INT128_MIN || parsed > INT128_MAX) {
      return {
        ok: false,
        error: 'Value out of int128 range',
      };
    }
    const uri = feedbackUri.trim();
    if (uri.length > MAX_URI_LENGTH) {
      return {
        ok: false,
        error: `Feedback URI must be at most ${MAX_URI_LENGTH} characters`,
      };
    }
    return { ok: true, value: parsed };
  };

  const handleReviewFeedback = () => {
    const result = validateForm();
    if (!result.ok) {
      addToast(result.error, 'error');
      return;
    }
    setConfirming(true);
  };

  const friendlyError = (err: unknown): string => {
    const msg = err instanceof Error ? err.message : String(err);
    // wagmi/viem common error codes
    if (/User rejected/i.test(msg) || /user denied/i.test(msg)) {
      return 'Transaction rejected in wallet';
    }
    if (/chain mismatch/i.test(msg) || /chain.*not.*configured/i.test(msg)) {
      return 'Please switch your wallet to Base and retry';
    }
    if (/insufficient funds/i.test(msg)) {
      return 'Insufficient funds to pay gas';
    }
    return msg;
  };

  const handleGiveFeedback = async () => {
    if (!address || !walletClient || !agentId) return;
    const result = validateForm();
    if (!result.ok) {
      addToast(result.error, 'error');
      return;
    }
    setSubmitting(true);
    try {
      await ensureBaseChain();

      const prepared = await api.prepareBaseGiveFeedback({
        from: address,
        agentId,
        value: result.value.toString(),
        valueDecimals: 0,
        tag1: ZERO_BYTES32,
        tag2: ZERO_BYTES32,
        endpoint: ZERO_BYTES32,
        feedbackUri: feedbackUri.trim(),
        feedbackHash: ZERO_BYTES32,
      });

      const hash = await walletClient.sendTransaction({
        account: address as `0x${string}`,
        to: prepared.to as `0x${string}`,
        data: prepared.data,
        value: BigInt(prepared.value),
      });

      await waitForTransactionReceipt(config, { hash });
      addToast('Feedback submitted', 'success');
      setShowForm(false);
      setConfirming(false);
      setFeedbackUri('');
      await fetchData({ silent: true });
    } catch (err) {
      addToast('Feedback failed: ' + friendlyError(err), 'error');
    } finally {
      setSubmitting(false);
    }
  };

  if (loading) {
    return (
      <Card>
        <CardContent className="py-6">
          <div className="animate-pulse text-text-secondary">
            Loading reputation…
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-4">
      <Card>
        <CardContent className="py-6 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <h2 className="text-lg font-semibold">Reputation</h2>
              {refreshing && (
                <span className="text-xs text-text-secondary animate-pulse">
                  refreshing…
                </span>
              )}
            </div>
            <ReputationBadge
              scoreBps={reputation?.scoreBps}
              confidenceBps={reputation?.confidenceBps}
            />
          </div>

          <div className="grid grid-cols-2 gap-3 text-sm">
            <div className="flex items-center justify-between border-b border-border-primary pb-2">
              <span className="text-text-secondary">Score</span>
              <span className="tabular-nums font-medium">
                {reputation?.scoreBps != null
                  ? `${(reputation.scoreBps / 100).toFixed(2)}%`
                  : '—'}
              </span>
            </div>
            <div className="flex items-center justify-between border-b border-border-primary pb-2">
              <span className="text-text-secondary">Confidence</span>
              <span className="tabular-nums font-medium">
                {reputation?.confidenceBps != null
                  ? `${(reputation.confidenceBps / 100).toFixed(0)}%`
                  : '—'}
              </span>
            </div>
            <div className="flex items-center justify-between border-b border-border-primary pb-2">
              <span className="text-text-secondary">Events</span>
              <span className="tabular-nums font-medium">
                {reputation?.events ?? '—'}
              </span>
            </div>
            <div className="flex items-center justify-between border-b border-border-primary pb-2">
              <span className="text-text-secondary">Notional</span>
              <span className="tabular-nums font-medium">
                {formatUsdc(reputation?.notionalMicrousdc)}
              </span>
            </div>
          </div>

          {canGiveFeedback && !showForm && (
            <Button
              variant="secondary"
              onClick={() => {
                setShowForm(true);
                setConfirming(false);
              }}
            >
              Give feedback
            </Button>
          )}

          {showForm && !confirming && (
            <div className="space-y-3 pt-2 border-t border-border-primary">
              <div className="space-y-1.5">
                <label className="block text-xs font-medium text-text-secondary">
                  Value (integer score)
                </label>
                <Input
                  type="number"
                  step="1"
                  value={valueInput}
                  onChange={(e) => setValueInput(e.target.value)}
                  placeholder="80"
                />
                <p className="text-[10px] text-text-secondary">
                  Integer in int128 range
                </p>
              </div>
              <div className="space-y-1.5">
                <label className="block text-xs font-medium text-text-secondary">
                  Feedback URI (optional)
                </label>
                <Input
                  type="text"
                  value={feedbackUri}
                  onChange={(e) => setFeedbackUri(e.target.value)}
                  placeholder="ipfs://…"
                  maxLength={MAX_URI_LENGTH}
                />
                <p className="text-[10px] text-text-secondary">
                  {feedbackUri.length}/{MAX_URI_LENGTH}
                </p>
              </div>
              <div className="flex items-center gap-2">
                <Button
                  variant="primary"
                  onClick={handleReviewFeedback}
                >
                  Review
                </Button>
                <Button
                  variant="secondary"
                  onClick={() => {
                    setShowForm(false);
                    setConfirming(false);
                  }}
                >
                  Cancel
                </Button>
              </div>
            </div>
          )}

          {showForm && confirming && (
            <div className="space-y-3 pt-2 border-t border-border-primary">
              <div className="rounded border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-400">
                On-chain feedback is permanent and attributed to your wallet.
                Confirm before signing.
              </div>
              <div className="grid grid-cols-2 gap-2 text-xs">
                <div className="text-text-secondary">Agent</div>
                <div className="font-mono">{agentId}</div>
                <div className="text-text-secondary">Value</div>
                <div className="font-mono">{valueInput}</div>
                <div className="text-text-secondary">URI</div>
                <div className="font-mono break-all">
                  {feedbackUri.trim() || '(none)'}
                </div>
              </div>
              <div className="flex items-center gap-2">
                <Button
                  variant="primary"
                  disabled={submitting}
                  onClick={handleGiveFeedback}
                >
                  {submitting ? 'Submitting…' : 'Sign & submit'}
                </Button>
                <Button
                  variant="secondary"
                  disabled={submitting}
                  onClick={() => setConfirming(false)}
                >
                  Back
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {feedback.length > 0 && (
        <Card>
          <CardContent className="py-6 space-y-3">
            <h3 className="text-sm font-semibold">
              Recent feedback ({feedback.length})
            </h3>
            <div className="space-y-2">
              {feedback.slice(0, 10).map((entry) => (
                <div
                  key={`${entry.client}-${entry.feedbackIndex}`}
                  className="flex items-center justify-between rounded border border-border-primary bg-bg-secondary/40 px-3 py-2 text-xs"
                >
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-text-secondary">
                      {formatAddress(entry.client)}
                    </span>
                    <span className="text-text-secondary">
                      #{entry.feedbackIndex}
                    </span>
                  </div>
                  <div className="flex items-center gap-2">
                    {entry.revoked && (
                      <span className="rounded bg-rose-500/20 px-1.5 py-0.5 text-[10px] text-rose-400">
                        revoked
                      </span>
                    )}
                    <span className="tabular-nums font-medium">
                      {formatFeedbackValue(entry)}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
