"use client";

import { PageShell } from "@/components/layout";
import { WalletPanel } from "@/components/wallet";

export default function WalletPage() {
  return (
    <PageShell>
      <div className="py-8">
        <div className="mx-auto max-w-3xl">
          <h1 className="text-2xl font-bold text-text-primary mb-6">Wallet</h1>
          <p className="mb-6 text-sm leading-6 text-text-secondary">
            Review vault balance, transaction history, and transfer actions. Live
            write actions still depend on wallet sign-in, runtime mode, and
            route-level availability.
          </p>
          <WalletPanel />
        </div>
      </div>
    </PageShell>
  );
}
