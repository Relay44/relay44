'use client';

import { useBaseWallet } from '@/hooks/useBaseWallet';
import { PageShell } from '@/components/layout';
import { Card, CardContent } from '@/components/ui/Card';
import { isAdminWallet } from '@/lib/admin';
import { SecurityAuditChecklist } from './SecurityAuditChecklist';

export function SecurityAuditPageClient() {
  const { address } = useBaseWallet();
  const isAdmin = isAdminWallet(address);

  if (!address) {
    return (
      <PageShell>
        <div className="container mx-auto max-w-6xl px-4 py-8">
          <Card>
            <CardContent className="flex h-40 items-center justify-center text-text-secondary">
              Connect your wallet to access security audit notes
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
      <div className="container mx-auto px-4 py-8">
        <h1 className="mb-6 text-2xl font-bold text-text-primary">Security Audit Preparation</h1>
        <SecurityAuditChecklist />
      </div>
    </PageShell>
  );
}
