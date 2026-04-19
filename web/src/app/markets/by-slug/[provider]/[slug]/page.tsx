"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { PageShell } from "@/components/layout";
import { LoadingScreen } from "@/components/ui";
import { resolveApiUrl } from "@/lib/api";

export default function MarketBySlugPage() {
  const router = useRouter();
  const params = useParams();
  const provider = String(params.provider ?? "");
  const slug = String(params.slug ?? "");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!provider || !slug) return;
    let cancelled = false;
    (async () => {
      try {
        const url = resolveApiUrl(
          `/evm/markets/by-slug/${encodeURIComponent(provider)}/${encodeURIComponent(slug)}`,
        );
        const res = await fetch(url, { cache: "no-store" });
        if (!res.ok) throw new Error(`resolve failed: ${res.status}`);
        const body = (await res.json()) as { canonicalId?: string };
        if (cancelled) return;
        if (body.canonicalId) {
          router.replace(`/markets/${encodeURIComponent(body.canonicalId)}`);
        } else {
          setError("market not found");
        }
      } catch (e) {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : "market not found");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [provider, slug, router]);

  if (error) {
    return (
      <PageShell>
        <div className="mx-auto max-w-xl py-16 text-center">
          <h1 className="text-lg font-semibold">Market not found</h1>
          <p className="mt-2 text-sm text-zinc-500">
            The link couldn&apos;t be resolved on Relay44.
          </p>
          <Link className="mt-6 inline-block underline" href="/markets">
            Browse all markets
          </Link>
        </div>
      </PageShell>
    );
  }

  return <LoadingScreen />;
}
