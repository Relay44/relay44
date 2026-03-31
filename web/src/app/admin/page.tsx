'use client';

import { useEffect, useMemo, useState } from 'react';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { api } from '@/lib/api';
import { isAdminWallet } from '@/lib/admin';
import { PageShell } from '@/components/layout';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import type { Market } from '@/types';

interface Stats {
  totalMarkets: number;
  activeMarkets: number;
  resolvedMarkets: number;
  totalVolume: number;
}

export default function AdminDashboard() {
  const { address } = useBaseWallet();
  const isAdmin = useMemo(() => isAdminWallet(address), [address]);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [stats, setStats] = useState<Stats>({
    totalMarkets: 0,
    activeMarkets: 0,
    resolvedMarkets: 0,
    totalVolume: 0,
  });
  const [recentMarkets, setRecentMarkets] = useState<Market[]>([]);

  useEffect(() => {
    if (!isAdmin) return;
    let mounted = true;

    const fetchAdminData = async () => {
      setLoading(true);
      setError(null);
      try {
        const page = await api.getBaseMarkets({ limit: 200, offset: 0, source: 'internal' });
        if (!mounted) return;

        const markets = page.data;
        const activeMarkets = markets.filter((m) => m.status === 'active').length;
        const resolvedMarkets = markets.filter((m) => m.status === 'resolved').length;
        const totalVolume = markets.reduce((sum, market) => sum + (market.totalVolume || 0), 0);

        setStats({
          totalMarkets: markets.length,
          activeMarkets,
          resolvedMarkets,
          totalVolume,
        });
        setRecentMarkets(markets.slice(0, 12));
      } catch (err) {
        if (!mounted) return;
        setError(err instanceof Error ? err.message : 'Failed to load admin data');
      } finally {
        if (mounted) setLoading(false);
      }
    };

    fetchAdminData();
    return () => {
      mounted = false;
    };
  }, [isAdmin]);

  if (!address) {
    return (
      <PageShell>
        <div className="container mx-auto max-w-6xl px-4 py-8">
          <Card>
            <CardContent className="flex h-40 items-center justify-center text-text-secondary">
              Connect your wallet to access admin dashboard
            </CardContent>
          </Card>
        </div>
      </PageShell>
    );
  }

  if (!isAdmin) {
    return (
      <PageShell>
        <div className="container mx-auto max-w-6xl px-4 py-8">
          <Card>
            <CardContent className="flex h-40 flex-col items-center justify-center gap-2">
              <p className="text-ask font-medium">Access denied</p>
              <p className="text-text-secondary text-sm">
                Wallet is not allowlisted for admin access.
              </p>
            </CardContent>
          </Card>
        </div>
      </PageShell>
    );
  }

  return (
    <PageShell>
      <div className="container mx-auto max-w-6xl space-y-8 px-4 py-8">
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-bold text-text-primary">Admin Dashboard</h1>
          <Badge variant="accent">Admin Mode</Badge>
        </div>

        <div className="grid grid-cols-2 gap-3 sm:gap-4 md:grid-cols-4">
          <StatCard label="Total Markets" value={stats.totalMarkets} />
          <StatCard label="Active Markets" value={stats.activeMarkets} />
          <StatCard label="Resolved Markets" value={stats.resolvedMarkets} />
          <StatCard label="Total Volume" value={`$${Math.round(stats.totalVolume)}`} />
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Recent Markets</CardTitle>
          </CardHeader>
          <CardContent>
            {loading ? (
              <p className="text-text-secondary">Loading markets...</p>
            ) : error ? (
              <p className="text-bid">{error}</p>
            ) : recentMarkets.length === 0 ? (
              <p className="text-text-secondary">No markets found</p>
            ) : (
              <div className="space-y-3">
                {recentMarkets.map((market) => (
                  <div
                    key={market.id}
                    className="flex items-center justify-between bg-bg-secondary px-4 py-3"
                  >
                    <div className="min-w-0">
                      <p className="text-text-primary font-medium truncate">{market.question}</p>
                      <p className="text-xs text-text-secondary mt-1">
                        {market.category} • closes {new Date(market.tradingEnd).toLocaleString()}
                      </p>
                    </div>
                    <Badge variant={market.status === 'resolved' ? 'success' : 'secondary'}>
                      {market.status}
                    </Badge>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </PageShell>
  );
}

function StatCard({ label, value }: { label: string; value: string | number }) {
  return (
    <Card>
      <CardContent className="p-4">
        <p className="text-sm text-text-secondary">{label}</p>
        <p className="mt-1 text-2xl font-semibold text-text-primary">{value}</p>
      </CardContent>
    </Card>
  );
}
