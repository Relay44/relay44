import { NextResponse } from 'next/server';
import { hasApiProxyTarget, resolveApiProxyTarget } from '@/lib/server/apiTarget';
import { readDetailedHealth, toApiErrorPayload } from '@/lib/server/baseReadApi';

export async function GET() {
  try {
    if (hasApiProxyTarget()) {
      const proxyTarget = resolveApiProxyTarget();
      const healthTarget = `${proxyTarget.replace(/\/v1\/?$/, '')}/health/detailed`;
      const capTarget = `${proxyTarget}/web4/capabilities`;

      try {
        const [healthRes, capRes] = await Promise.all([
          fetch(healthTarget, { cache: 'no-store', signal: AbortSignal.timeout(5_000) }),
          fetch(capTarget, { cache: 'no-store', signal: AbortSignal.timeout(5_000) }).catch(
            () => null,
          ),
        ]);

        if (healthRes.ok) {
          const health = (await healthRes.json()) as Record<string, unknown>;

          // Merge backend capabilities into the health response so both
          // endpoints agree on launch readiness and runtime flags.
          if (capRes?.ok) {
            try {
              health.capabilities = await capRes.json();
            } catch {
              // Non-fatal — capabilities field will just be absent.
            }
          }

          return NextResponse.json(health);
        }
      } catch {
        // Fall back to local health when the API detailed check is slow or unavailable.
      }
    }
    return NextResponse.json(await readDetailedHealth());
  } catch (error) {
    const mapped = toApiErrorPayload(error);
    return NextResponse.json(mapped.payload, { status: mapped.status });
  }
}
