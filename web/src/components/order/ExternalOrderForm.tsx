'use client';

import Link from 'next/link';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Button, Card, Input, Select, useToast } from '@/components/ui';
import { ReadOnlyNotice } from '@/components/runtime/ReadOnlyNotice';
import {
  api,
  type ExternalCredential,
  type ExternalCredentialStatus,
  type ExternalOrderRecord,
} from '@/lib/api';
import {
  cancelExternalMarketOrder,
  submitExternalMarketOrder,
} from '@/lib/externalExecution';
import { useRuntimeMode, useSessionState } from '@/hooks';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import type { Market, Outcome, OrderSide } from '@/types';
import { cn } from '@/lib/utils';

export interface ExternalOrderFormProps {
  market: Market;
  onSuccess?: () => void;
}

function providerFromMarket(market: Market): 'limitless' | 'polymarket' {
  return market.provider === 'polymarket' ? 'polymarket' : 'limitless';
}

function isPolymarketOrderBelowMinimumNotional(
  provider: 'limitless' | 'polymarket',
  price: number,
  quantity: number
): boolean {
  return provider === 'polymarket' && price * quantity < 1;
}

export function ExternalOrderForm({ market, onSuccess }: ExternalOrderFormProps) {
  const { addToast } = useToast();
  const baseWallet = useBaseWallet();
  const { readOnly } = useRuntimeMode();
  const { hasSession, sessionRestored } = useSessionState();
  const canManageCredentials = sessionRestored && hasSession;
  const [outcome, setOutcome] = useState<Outcome>('yes');
  const [side, setSide] = useState<OrderSide>('buy');
  const [price, setPrice] = useState(String(Math.round(market.yesPrice * 100) / 100));
  const [quantity, setQuantity] = useState('10');
  const [credentialId, setCredentialId] = useState('');
  const [credentials, setCredentials] = useState<ExternalCredential[]>([]);
  const [credentialStatus, setCredentialStatus] = useState<ExternalCredentialStatus | null>(null);
  const [signedOrderJson, setSignedOrderJson] = useState('');
  const [isLoadingCredentials, setIsLoadingCredentials] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [preflight, setPreflight] = useState<Record<string, unknown> | null>(null);
  const [lastOrder, setLastOrder] = useState<ExternalOrderRecord | null>(null);
  const [recentOrders, setRecentOrders] = useState<ExternalOrderRecord[]>([]);
  const [isLoadingOrders, setIsLoadingOrders] = useState(false);
  const [cancellingOrderId, setCancellingOrderId] = useState<string | null>(null);

  const provider = providerFromMarket(market);
  const currentPrice = outcome === 'yes' ? market.yesPrice : market.noPrice;

  useEffect(() => {
    if (readOnly || !canManageCredentials) {
      setCredentials([]);
      setCredentialId('');
      return;
    }

    let canceled = false;

    async function load() {
      setIsLoadingCredentials(true);
      try {
        const list = await api.getExternalCredentials(provider);
        if (canceled) return;
        setCredentials(list);
        if (!credentialId && list.length > 0) {
          setCredentialId(list[0].id);
        }
      } catch (error) {
        if (!canceled) {
          const message = error instanceof Error ? error.message : 'Failed to load credentials';
          addToast(message, 'error');
        }
      } finally {
        if (!canceled) {
          setIsLoadingCredentials(false);
        }
      }
    }

    void load();

    return () => {
      canceled = true;
    };
  }, [addToast, canManageCredentials, credentialId, provider, readOnly]);

  useEffect(() => {
    if (readOnly || !canManageCredentials || credentials.length === 0) {
      setCredentialStatus(null);
      return;
    }

    let canceled = false;

    async function loadStatus() {
      try {
        const status = await api.getExternalCredentialStatus(provider, credentialId || undefined);
        if (!canceled) {
          setCredentialStatus(status);
        }
      } catch (error) {
        if (!canceled) {
          const message =
            error instanceof Error ? error.message : 'Failed to load credential readiness';
          addToast(message, 'error');
        }
      }
    }

    void loadStatus();

    return () => {
      canceled = true;
    };
  }, [addToast, canManageCredentials, credentialId, credentials.length, provider, readOnly]);

  const preflightChecks = useMemo(() => {
    const checks = preflight?.checks;
    return Array.isArray(checks) ? checks : [];
  }, [preflight]);
  const credentialOptions = useMemo(
    () =>
      credentials.map((entry) => ({
        value: entry.id,
        label: entry.label,
      })),
    [credentials]
  );
  const loadRecentOrders = useCallback(async () => {
    if (readOnly || !canManageCredentials) {
      setRecentOrders([]);
      return;
    }

    setIsLoadingOrders(true);
    try {
      const response = await api.listExternalOrders({ provider, limit: 20 });
      setRecentOrders(response.orders.filter((entry) => entry.market_id === market.id).slice(0, 6));
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to load external orders';
      addToast(message, 'error');
    } finally {
      setIsLoadingOrders(false);
    }
  }, [addToast, canManageCredentials, market.id, provider, readOnly]);

  useEffect(() => {
    void loadRecentOrders();
  }, [loadRecentOrders]);

  if (readOnly) {
    return (
      <ReadOnlyNotice
        title="External trading is currently unavailable"
        body="External venue data remains available."
      />
    );
  }

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();

    const numericPrice = Number(price || currentPrice);
    const numericQuantity = Number(quantity);
    if (!Number.isFinite(numericPrice) || numericPrice <= 0 || numericPrice >= 1) {
      addToast('Price must be between 0 and 1', 'error');
      return;
    }
    if (!Number.isFinite(numericQuantity) || numericQuantity <= 0) {
      addToast('Quantity must be greater than zero', 'error');
      return;
    }
    if (isPolymarketOrderBelowMinimumNotional(provider, numericPrice, numericQuantity)) {
      addToast('Polymarket minimum order size is $1 notional', 'error');
      return;
    }
    if (!credentialId) {
      addToast('Select a credential first', 'error');
      return;
    }
    if (!canManageCredentials) {
      addToast('Authenticate before using venue credentials', 'error');
      return;
    }
    if (credentialStatus && !credentialStatus.ready) {
      addToast('Selected credential is not ready for live execution', 'error');
      return;
    }

    setIsSubmitting(true);
    try {
      const { intent, order } = await submitExternalMarketOrder({
        provider,
        marketId: market.id,
        outcome,
        side,
        price: numericPrice,
        quantity: numericQuantity,
        credentialId,
        walletAddress: baseWallet.address || '',
        signedOrderJson,
      });
      setPreflight(intent.preflight || null);
      setLastOrder(order);
      await loadRecentOrders();
      addToast('External order submitted', 'success');
      onSuccess?.();
    } catch (error) {
      const message = error instanceof Error ? error.message : 'External order submit failed';
      addToast(message, 'error');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancel = async (order: ExternalOrderRecord) => {
    if (!credentialId) {
      addToast('Select a credential first', 'error');
      return;
    }

    setCancellingOrderId(order.id);
    try {
      await cancelExternalMarketOrder({
        provider,
        providerOrderId: order.provider_order_id,
        credentialId,
      });
      setLastOrder((current) =>
        current?.id === order.id ? { ...current, status: 'cancelled' } : current
      );
      await loadRecentOrders();
      addToast('External order cancelled', 'success');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'External order cancel failed';
      addToast(message, 'error');
    } finally {
      setCancellingOrderId(null);
    }
  };

  return (
    <Card className="!p-4 sm:!p-6">
      <h3 className="font-display font-semibold text-lg mb-4">External Trade</h3>
      <p className="text-xs text-text-muted mb-4">
        Venue: {provider} · Chain {market.chainId}
      </p>

      {provider === 'polymarket' ? (
        <div className="mb-4 border border-border p-3 text-xs text-text-secondary">
          Polymarket orders use your saved CLOB credentials plus a wallet signature from the
          connected account. Browser-wallet accounts need the correct funder wallet and signature
          type.
        </div>
      ) : null}

      {!canManageCredentials ? (
        <div className="mb-4 border border-border p-3 text-xs text-text-secondary">
          Authenticate your wallet session before loading venue credentials or submitting external
          orders.
        </div>
      ) : null}

      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="grid grid-cols-2 gap-2">
          <button
            type="button"
            onClick={() => setOutcome('yes')}
            className={cn(
              'py-2 border text-sm',
              outcome === 'yes' ? 'border-bid text-bid bg-bid-muted' : 'border-border text-text-secondary'
            )}
          >
            YES
          </button>
          <button
            type="button"
            onClick={() => setOutcome('no')}
            className={cn(
              'py-2 border text-sm',
              outcome === 'no' ? 'border-ask text-ask bg-ask-muted' : 'border-border text-text-secondary'
            )}
          >
            NO
          </button>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <button
            type="button"
            onClick={() => setSide('buy')}
            className={cn('py-2 border text-sm', side === 'buy' ? 'border-accent text-accent' : 'border-border text-text-secondary')}
          >
            Buy
          </button>
          <button
            type="button"
            onClick={() => setSide('sell')}
            className={cn('py-2 border text-sm', side === 'sell' ? 'border-accent text-accent' : 'border-border text-text-secondary')}
          >
            Sell
          </button>
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium text-text-primary">Credential</label>
          <Select
            value={credentialId || undefined}
            onChange={(event) => setCredentialId(event.target.value)}
            options={credentialOptions}
            placeholder={isLoadingCredentials ? 'Loading credentials...' : 'Select credential'}
            disabled={isLoadingCredentials || !canManageCredentials}
          />
        </div>

        {credentialStatus ? (
          <div className="border border-border p-3 text-xs text-text-secondary">
            <div className="font-medium text-text-primary">
              {credentialStatus.ready ? 'Credential ready' : 'Credential not ready'}
            </div>
            {credentialStatus.base_wallet ? (
              <div className="mt-1">Base wallet: {credentialStatus.base_wallet}</div>
            ) : null}
            {credentialStatus.checks
              .filter((check) => !check.ok)
              .map((check) => (
                <div key={check.code} className="mt-1">
                  {check.message}
                </div>
              ))}
            {!credentialStatus.ready ? (
              <div className="mt-2">
                <Link href="/settings/credentials" className="text-accent hover:text-accent-hover">
                  Fix credential readiness
                </Link>
              </div>
            ) : null}
          </div>
        ) : null}

        <Input
          type="number"
          label="Price"
          value={price}
          onChange={(event) => setPrice(event.target.value)}
          min="0.01"
          max="0.99"
          step="0.01"
          placeholder={String(currentPrice)}
        />

        <Input
          type="number"
          label="Quantity"
          value={quantity}
          onChange={(event) => setQuantity(event.target.value)}
          min="0.01"
          step="0.01"
        />

        <Input
          label="Signed Order JSON (optional)"
          value={signedOrderJson}
          onChange={(event) => setSignedOrderJson(event.target.value)}
          placeholder='{"typedData":{...},"signature":"0x..."}'
        />

        <Button
          type="submit"
          className="w-full"
          loading={isSubmitting}
          disabled={
            !canManageCredentials ||
            isSubmitting ||
            !credentialId ||
            (credentialStatus ? !credentialStatus.ready : false)
          }
        >
          Submit External Order
        </Button>
      </form>

      {credentials.length > 0 ? (
        <div className="mt-4 text-xs text-text-muted">
          Credentials loaded: {credentials.length}
        </div>
      ) : (
        <div className="mt-4 text-xs text-text-muted">
          No credentials saved.{' '}
          <Link href="/settings/credentials" className="text-accent hover:text-accent-hover">
            Add venue credentials
          </Link>
          .
        </div>
      )}

      {preflightChecks.length > 0 ? (
        <div className="mt-4 space-y-2">
          <p className="text-xs text-text-muted">Preflight checks</p>
          {preflightChecks.map((entry, index) => (
            <div key={index} className="text-xs border border-border p-2">
              {String((entry as { message?: string }).message || 'Check')}
            </div>
          ))}
        </div>
      ) : null}

      {lastOrder ? (
        <div className="mt-4 text-xs border border-border p-2">
          <div>Status: {lastOrder.status}</div>
          <div>Provider Order: {lastOrder.provider_order_id || 'n/a'}</div>
        </div>
      ) : null}

      {recentOrders.length > 0 ? (
        <div className="mt-4 space-y-2">
          <p className="text-xs text-text-muted">Recent external orders</p>
          {recentOrders.map((order) => {
            const canCancel = order.status === 'submitted' && !!order.provider_order_id;
            return (
              <div
                key={order.id}
                className="flex items-center justify-between gap-3 border border-border p-2 text-xs"
              >
                <div className="min-w-0">
                  <div className="truncate text-text-primary">
                    {order.status} · {order.provider_order_id || 'pending id'}
                  </div>
                  <div className="text-text-muted">
                    {new Date(order.created_at).toLocaleString()}
                  </div>
                </div>
                {canCancel ? (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    loading={cancellingOrderId === order.id}
                    onClick={() => void handleCancel(order)}
                  >
                    Cancel
                  </Button>
                ) : null}
              </div>
            );
          })}
        </div>
      ) : isLoadingOrders ? (
        <div className="mt-4 text-xs text-text-muted">Loading recent orders...</div>
      ) : null}
    </Card>
  );
}
