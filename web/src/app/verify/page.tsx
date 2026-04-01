"use client";

import { useCallback, useEffect, useState } from "react";
import { PageShell } from "@/components/layout";
import { Button, Card, CardContent, useToast } from "@/components/ui";
import { useBaseWallet } from "@/hooks/useBaseWallet";
import { api } from "@/lib/api";
import { KycTierBadge } from "@/components/ui/KycTierBadge";
import type { KycStatus } from "@/types";

export default function VerifyPage() {
  const { address, isConnected } = useBaseWallet();
  const { addToast } = useToast();

  const [status, setStatus] = useState<KycStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [verifying, setVerifying] = useState(false);

  const fetchStatus = useCallback(async () => {
    if (!address) {
      setStatus(null);
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      const result = await api.getKycStatus();
      setStatus(result);
    } catch {
      setStatus(null);
    } finally {
      setLoading(false);
    }
  }, [address]);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  const handleVerify = async () => {
    if (!address) return;
    setVerifying(true);
    try {
      // World ID IDKit widget integration placeholder.
      // When IDKit is installed, the widget returns merkle_root, nullifier_hash, proof.
      // Those are then passed to api.verifyKyc().
      addToast(
        "World ID verification requires the IDKit widget. Connect via World ID Developer Portal.",
        "info",
      );
    } catch (err) {
      addToast(
        err instanceof Error ? err.message : "Verification failed",
        "error",
      );
    } finally {
      setVerifying(false);
    }
  };

  const isVerified = (status?.tier ?? 0) >= 2;

  return (
    <PageShell>
      <div className="mx-auto max-w-xl space-y-6">
        <h1 className="text-xl font-semibold text-text-primary">
          Identity Verification
        </h1>

        <Card>
          <CardContent className="space-y-4 p-6">
            <h2 className="text-lg font-semibold text-text-primary">
              KYC Tier Status
            </h2>

            {!isConnected ? (
              <p className="text-sm text-text-secondary">
                Connect your wallet to view your verification status.
              </p>
            ) : loading ? (
              <p className="text-sm text-text-secondary">Loading...</p>
            ) : (
              <>
                <div className="flex items-center gap-3">
                  <KycTierBadge tier={status?.tier ?? 0} />
                  <div>
                    <p className="text-sm font-medium text-text-primary">
                      {isVerified ? "Verified" : "Unverified"}
                    </p>
                    {status?.verifiedAt && (
                      <p className="text-xs text-text-tertiary">
                        Verified{" "}
                        {new Date(status.verifiedAt).toLocaleDateString()}
                      </p>
                    )}
                  </div>
                </div>

                {isVerified ? (
                  <div className="rounded-lg border border-green-500/20 bg-green-500/5 p-4">
                    <p className="text-sm text-green-400">
                      Your identity has been verified. You can trade on
                      KYC-gated markets and have higher position limits.
                    </p>
                  </div>
                ) : (
                  <div className="space-y-3">
                    <div className="rounded-lg border border-border/50 bg-bg-secondary/50 p-4">
                      <h3 className="text-sm font-medium text-text-primary">
                        Why verify?
                      </h3>
                      <ul className="mt-2 space-y-1 text-xs text-text-secondary">
                        <li>Access KYC-gated markets</li>
                        <li>Higher position limits ($100K vs $1K)</li>
                        <li>Verified badge on your profile</li>
                      </ul>
                    </div>

                    <Button
                      onClick={handleVerify}
                      disabled={verifying}
                      className="w-full"
                    >
                      {verifying ? "Verifying..." : "Verify with World ID"}
                    </Button>

                    <p className="text-center text-xs text-text-tertiary">
                      Verification uses World ID zero-knowledge proofs. No
                      personal data is stored.
                    </p>
                  </div>
                )}
              </>
            )}
          </CardContent>
        </Card>
      </div>
    </PageShell>
  );
}
