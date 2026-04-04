"use client";

import dynamic from "next/dynamic";
import { useCallback, useEffect, useState } from "react";
import { PageShell } from "@/components/layout";
import { Card, CardContent, useToast } from "@/components/ui";
import { useBaseWallet } from "@/hooks/useBaseWallet";
import { api } from "@/lib/api";
import { KycTierBadge } from "@/components/ui/KycTierBadge";
import type { KycStatus } from "@/types";

const IDKitWidget = dynamic(
  () => import("@worldcoin/idkit").then((mod) => mod.IDKitWidget),
  { ssr: false },
);

type IDKitResult = {
  proof: string;
  merkle_root: string;
  nullifier_hash: string;
  verification_level: string;
};

const WORLD_ID_APP_ID = process.env.NEXT_PUBLIC_WORLD_ID_APP_ID as
  | `app_${string}`
  | undefined;
const WORLD_ID_ACTION_ID =
  process.env.NEXT_PUBLIC_WORLD_ID_ACTION_ID || "verify-relay44";

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

  const handleVerify = async (result: IDKitResult) => {
    if (!address) return;
    setVerifying(true);
    try {
      const response = await api.verifyKyc({
        merkle_root: result.merkle_root,
        nullifier_hash: result.nullifier_hash,
        proof: result.proof,
        action_id: WORLD_ID_ACTION_ID,
        signal: address.toLowerCase(),
      });
      setStatus((prev) =>
        prev
          ? { ...prev, tier: response.tier as KycStatus["tier"], provider: "world_id" }
          : prev,
      );
      addToast("Identity verified successfully", "success");
      await fetchStatus();
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
  const idkitReady = Boolean(WORLD_ID_APP_ID && address && !isVerified);

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

                    {!WORLD_ID_APP_ID ? (
                      <div className="rounded-lg border border-yellow-500/20 bg-yellow-500/5 p-4">
                        <p className="text-sm text-yellow-400">
                          World ID verification is not configured on this
                          instance. Contact the operator to enable KYC.
                        </p>
                      </div>
                    ) : idkitReady ? (
                      <IDKitWidget
                        app_id={WORLD_ID_APP_ID!}
                        action={WORLD_ID_ACTION_ID}
                        signal={address!.toLowerCase()}
                        onSuccess={handleVerify}
                        autoClose
                      >
                        {({ open }: { open: () => void }) => (
                          <button
                            type="button"
                            onClick={open}
                            disabled={verifying}
                            className="w-full h-10 border border-accent text-accent text-[0.7rem] uppercase tracking-[0.12em] font-mono transition-colors hover:bg-accent/10 disabled:opacity-50"
                          >
                            {verifying ? "Verifying..." : "Verify with World ID"}
                          </button>
                        )}
                      </IDKitWidget>
                    ) : null}

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
