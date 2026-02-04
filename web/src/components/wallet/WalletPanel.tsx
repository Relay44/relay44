"use client";

import Link from "next/link";
import { useState, useEffect } from "react";
import { useRuntimeMode, useSessionState } from "@/hooks";
import { ReadOnlyNotice } from "@/components/runtime/ReadOnlyNotice";
import { api } from "@/lib/api";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/Tabs";
import type { WalletBalance } from "@/types";
import { DepositForm } from "./DepositForm";
import { WithdrawForm } from "./WithdrawForm";
import { TransactionHistory } from "./TransactionHistory";

function formatUsdc(amount: number): string {
  return (amount / 1_000_000).toFixed(2);
}

export function WalletPanel() {
  const [balance, setBalance] = useState<WalletBalance | null>(null);
  const [tab, setTab] = useState<'deposit' | 'withdraw' | 'history'>('deposit');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { readOnly, capabilities } = useRuntimeMode();
  const { hasSession, sessionRestored } = useSessionState();
  const walletRuntime = capabilities?.wallet;
  const depositEnabled = !readOnly && (walletRuntime?.deposit_enabled ?? false);
  const withdrawEnabled = !readOnly && (walletRuntime?.withdraw_enabled ?? false);
  const transferNotice =
    readOnly
      ? 'Balance and history remain visible, but deposits and withdrawals are disabled in this environment.'
      : !depositEnabled && !withdrawEnabled
        ? 'Deposits and withdrawals are unavailable in the current runtime configuration.'
        : !depositEnabled
          ? 'Deposits are unavailable in the current runtime configuration.'
          : !withdrawEnabled
            ? 'Withdrawals are unavailable in the current runtime configuration.'
            : null;

  const fetchBalance = async () => {
    try {
      setLoading(true);
      const data = await api.getWalletBalance();
      setBalance(data);
      setError(null);
    } catch (err) {
      setError("Failed to load balance");
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!sessionRestored) {
      return;
    }

    if (!hasSession) {
      setBalance(null);
      setError(null);
      setLoading(false);
      return;
    }

    void fetchBalance();
  }, [hasSession, sessionRestored]);

  useEffect(() => {
    if (tab === 'deposit' && !depositEnabled) {
      setTab(withdrawEnabled ? 'withdraw' : 'history');
      return;
    }
    if (tab === 'withdraw' && !withdrawEnabled) {
      setTab(depositEnabled ? 'deposit' : 'history');
    }
  }, [depositEnabled, tab, withdrawEnabled]);

  if (!sessionRestored) {
    return (
      <Card>
        <CardContent className="flex items-center justify-center h-48">
          <div className="animate-pulse text-text-secondary">Loading...</div>
        </CardContent>
      </Card>
    );
  }

  if (loading) {
    return (
      <Card>
        <CardContent className="flex items-center justify-center h-48">
          <div className="animate-pulse text-text-secondary">Loading...</div>
        </CardContent>
      </Card>
    );
  }

  if (error) {
    return (
      <Card>
        <CardContent className="flex flex-col items-center justify-center h-48 gap-4">
          <p className="text-text-secondary">{error}</p>
          <Button variant="secondary" onClick={fetchBalance}>
            Retry
          </Button>
        </CardContent>
      </Card>
    );
  }

  if (!hasSession) {
    return (
      <Card>
        <CardContent className="flex flex-col items-center justify-center h-48 gap-2 text-center">
          <p className="font-medium text-text-primary">
            Wallet sign-in required
          </p>
          <p className="max-w-md text-sm text-text-secondary">
            Connect your Base wallet from the header, approve the sign-in
            prompt, and then return here to inspect balances, deposits,
            withdrawals, and transaction history.
          </p>
          <div className="mt-3 flex flex-wrap justify-center gap-3">
            <Link
              href="/how-it-works"
              className="inline-flex h-10 items-center border border-accent px-4 text-sm uppercase tracking-[0.12em] text-accent transition-colors hover:bg-accent/10"
            >
              How it works
            </Link>
            <Link
              href="/markets"
              className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
            >
              Browse markets
            </Link>
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      {/* Balance Card */}
      <Card>
        <CardHeader>
          <CardTitle>Wallet Balance</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-4 lg:grid-cols-5">
            <div>
              <p className="text-sm text-text-secondary">Available</p>
              <p className="text-2xl font-semibold text-text-primary">
                ${formatUsdc(balance?.available || 0)}
              </p>
            </div>
            <div>
              <p className="text-sm text-text-secondary">Locked</p>
              <p className="text-xl font-medium text-text-secondary">
                ${formatUsdc(balance?.locked || 0)}
              </p>
            </div>
            <div>
              <p className="text-sm text-text-secondary">Pending In</p>
              <p className="text-xl font-medium text-bid">
                +${formatUsdc(balance?.pendingDeposits || 0)}
              </p>
            </div>
            <div>
              <p className="text-sm text-text-secondary">Pending Out</p>
              <p className="text-xl font-medium text-ask">
                -${formatUsdc(balance?.pendingWithdrawals || 0)}
              </p>
            </div>
            <div>
              <p className="text-sm text-text-secondary">Claimable</p>
              <p className="text-xl font-medium text-accent">
                ${formatUsdc(balance?.claimable || 0)}
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Deposit/Withdraw Tabs */}
      <Card>
        <Tabs value={tab} onValueChange={(value) => setTab(value as typeof tab)} className="w-full">
          <CardHeader className="pb-0">
            {readOnly ? (
              <ReadOnlyNotice
                title="Wallet actions are disabled"
                body={transferNotice || 'Wallet actions are disabled in this environment.'}
              />
            ) : transferNotice ? (
              <p className="mb-4 text-sm text-text-secondary">{transferNotice}</p>
            ) : null}
            <TabsList className="grid w-full grid-cols-3">
              <TabsTrigger value="deposit" disabled={!depositEnabled}>
                Deposit
              </TabsTrigger>
              <TabsTrigger value="withdraw" disabled={!withdrawEnabled}>
                Withdraw
              </TabsTrigger>
              <TabsTrigger value="history">History</TabsTrigger>
            </TabsList>
          </CardHeader>
          <CardContent className="pt-6">
            <TabsContent value="deposit">
              {depositEnabled ? (
                <DepositForm onSuccess={fetchBalance} />
              ) : (
                <p className="text-sm text-text-secondary">
                  Wallet funding is disabled for the current runtime configuration.
                </p>
              )}
            </TabsContent>
            <TabsContent value="withdraw">
              {withdrawEnabled ? (
                <WithdrawForm
                  availableBalance={balance?.available || 0}
                  onSuccess={fetchBalance}
                />
              ) : (
                <p className="text-sm text-text-secondary">
                  Wallet payouts are disabled for the current runtime configuration.
                </p>
              )}
            </TabsContent>
            <TabsContent value="history">
              <TransactionHistory />
            </TabsContent>
          </CardContent>
        </Tabs>
      </Card>
    </div>
  );
}

