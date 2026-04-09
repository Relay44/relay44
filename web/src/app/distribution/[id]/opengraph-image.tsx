import { ImageResponse } from "next/og";

export const runtime = "edge";

export const size = { width: 1200, height: 630 };

export const contentType = "image/png";

const API_BASE =
  process.env.NEXT_PUBLIC_API_URL?.trim() ||
  "https://relay44-api.onrender.com";

interface DistMarketData {
  question: string;
  description?: string;
  category?: string;
  status: string;
  outcomeMin: number;
  outcomeMax: number;
  outcomeUnit?: string;
  marketMu?: number;
  marketSigma?: number;
  totalCollateral: number;
  totalVolume: number;
  tradingEnd?: string;
}

function formatVolume(value: number): string {
  if (value >= 1_000_000) return `$${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `$${(value / 1_000).toFixed(1)}K`;
  return `$${value.toFixed(0)}`;
}

function statusLabel(status: string): { text: string; color: string } {
  switch (status) {
    case "active":
      return { text: "LIVE", color: "#22c55e" };
    case "paused":
      return { text: "PAUSED", color: "#f59e0b" };
    case "closed":
      return { text: "CLOSED", color: "#f59e0b" };
    case "resolved":
      return { text: "RESOLVED", color: "#8b5cf6" };
    case "cancelled":
      return { text: "CANCELLED", color: "#ef4444" };
    default:
      return { text: status.toUpperCase(), color: "#71717a" };
  }
}

/** Draw a simplified bell curve for the OG card. */
function bellCurvePoints(
  mu: number,
  sigma: number,
  min: number,
  max: number,
): Array<{ x: number; y: number }> {
  const points: Array<{ x: number; y: number }> = [];
  const steps = 60;
  const rangeMin = Math.max(min, mu - 3.5 * sigma);
  const rangeMax = Math.min(max, mu + 3.5 * sigma);
  const step = (rangeMax - rangeMin) / steps;

  let peakY = 0;
  for (let i = 0; i <= steps; i++) {
    const xVal = rangeMin + i * step;
    const z = (xVal - mu) / sigma;
    const y = Math.exp(-0.5 * z * z);
    if (y > peakY) peakY = y;
    points.push({ x: xVal, y });
  }

  // Normalize y values to 0-1
  return points.map((p) => ({ x: p.x, y: peakY > 0 ? p.y / peakY : 0 }));
}

function curveSvgPath(
  points: Array<{ x: number; y: number }>,
  width: number,
  height: number,
  min: number,
  max: number,
): string {
  if (points.length === 0) return "";
  const range = max - min || 1;
  const pad = 10;
  const usableW = width - pad * 2;
  const usableH = height - pad * 2;

  const mapped = points.map((p) => ({
    px: pad + ((p.x - min) / range) * usableW,
    py: pad + (1 - p.y) * usableH,
  }));

  let d = `M ${mapped[0].px} ${mapped[0].py}`;
  for (let i = 1; i < mapped.length; i++) {
    d += ` L ${mapped[i].px} ${mapped[i].py}`;
  }

  return d;
}

export default async function Image({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  let market: DistMarketData | null = null;

  try {
    const res = await fetch(
      `${API_BASE}/v1/distribution/markets/${encodeURIComponent(id)}`,
      { next: { revalidate: 300 } },
    );
    if (res.ok) {
      const json = await res.json();
      market = json.data ?? json;
    }
  } catch {
    // fall through
  }

  if (!market) {
    return new ImageResponse(
      (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            width: "100%",
            height: "100%",
            backgroundColor: "#030303",
            padding: "60px",
          }}
        >
          <div
            style={{
              fontFamily: "monospace",
              fontSize: 48,
              fontWeight: 700,
              color: "#ffffff",
              marginBottom: 24,
            }}
          >
            relay44
          </div>
          <div style={{ fontSize: 32, color: "#a1a1aa", textAlign: "center" }}>
            Distribution market not found
          </div>
        </div>
      ),
      { ...size },
    );
  }

  const status = statusLabel(market.status);
  const mu = market.marketMu ?? (market.outcomeMin + market.outcomeMax) / 2;
  const sigma = market.marketSigma ?? (market.outcomeMax - market.outcomeMin) / 6;
  const collateral = formatVolume(market.totalCollateral || 0);
  const volume = formatVolume(market.totalVolume || 0);
  const unit = market.outcomeUnit || "";

  // Build bell curve SVG for visual
  const curveW = 480;
  const curveH = 160;
  const curvePoints = bellCurvePoints(mu, sigma, market.outcomeMin, market.outcomeMax);
  const pathD = curveSvgPath(curvePoints, curveW, curveH, market.outcomeMin, market.outcomeMax);

  return new ImageResponse(
    (
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          width: "100%",
          height: "100%",
          backgroundColor: "#030303",
          padding: "48px 60px",
          position: "relative",
        }}
      >
        {/* Top bar: branding + status + category */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            width: "100%",
            marginBottom: 28,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
            <div
              style={{
                fontFamily: "monospace",
                fontSize: 32,
                fontWeight: 700,
                color: "#ffffff",
                letterSpacing: "-0.02em",
              }}
            >
              relay44
            </div>
            <div
              style={{
                display: "flex",
                fontSize: 12,
                fontWeight: 600,
                color: "#a78bfa",
                border: "1px solid #a78bfa40",
                backgroundColor: "#a78bfa15",
                padding: "4px 12px",
                borderRadius: 4,
                letterSpacing: "0.08em",
                textTransform: "uppercase",
              }}
            >
              Distribution
            </div>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 6,
                fontSize: 14,
                fontWeight: 600,
                color: status.color,
                border: `1px solid ${status.color}40`,
                backgroundColor: `${status.color}15`,
                padding: "4px 12px",
                borderRadius: 4,
                letterSpacing: "0.06em",
              }}
            >
              <div
                style={{
                  width: 6,
                  height: 6,
                  borderRadius: 3,
                  backgroundColor: status.color,
                }}
              />
              {status.text}
            </div>
          </div>
          {market.category ? (
            <div
              style={{
                display: "flex",
                fontSize: 16,
                color: "#a1a1aa",
                border: "1px solid #27272a",
                padding: "6px 16px",
                borderRadius: 4,
                textTransform: "uppercase",
                letterSpacing: "0.08em",
              }}
            >
              {market.category}
            </div>
          ) : null}
        </div>

        {/* Main content: question + curve side by side */}
        <div
          style={{
            display: "flex",
            flex: 1,
            alignItems: "center",
            gap: 48,
          }}
        >
          {/* Left: question */}
          <div
            style={{
              display: "flex",
              flex: 1,
              flexDirection: "column",
              justifyContent: "center",
            }}
          >
            <div
              style={{
                fontSize: market.question.length > 80 ? 32 : 38,
                fontWeight: 600,
                color: "#ffffff",
                lineHeight: 1.3,
                overflow: "hidden",
                textOverflow: "ellipsis",
                display: "-webkit-box",
                WebkitLineClamp: 3,
                WebkitBoxOrient: "vertical",
              }}
            >
              {market.question}
            </div>
          </div>

          {/* Right: bell curve + stats */}
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              width: curveW,
              flexShrink: 0,
            }}
          >
            {/* SVG bell curve */}
            <svg
              width={curveW}
              height={curveH}
              viewBox={`0 0 ${curveW} ${curveH}`}
              style={{ marginBottom: 16 }}
            >
              <defs>
                <linearGradient id="curveGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="#a78bfa" stopOpacity="0.4" />
                  <stop offset="100%" stopColor="#a78bfa" stopOpacity="0.05" />
                </linearGradient>
              </defs>
              {/* Fill area */}
              <path
                d={`${pathD} L ${curveW - 10} ${curveH - 10} L 10 ${curveH - 10} Z`}
                fill="url(#curveGrad)"
              />
              {/* Curve line */}
              <path
                d={pathD}
                fill="none"
                stroke="#a78bfa"
                strokeWidth="3"
              />
            </svg>

            {/* Mu / Sigma stats */}
            <div
              style={{
                display: "flex",
                gap: 32,
                justifyContent: "center",
              }}
            >
              <div style={{ display: "flex", flexDirection: "column", alignItems: "center" }}>
                <span style={{ fontSize: 13, color: "#71717a", letterSpacing: "0.06em" }}>
                  MEAN (\u03BC)
                </span>
                <span
                  style={{
                    fontFamily: "monospace",
                    fontSize: 28,
                    fontWeight: 700,
                    color: "#ffffff",
                    marginTop: 4,
                  }}
                >
                  {mu.toFixed(1)}{unit ? ` ${unit}` : ""}
                </span>
              </div>
              <div style={{ display: "flex", flexDirection: "column", alignItems: "center" }}>
                <span style={{ fontSize: 13, color: "#71717a", letterSpacing: "0.06em" }}>
                  STD DEV (\u03C3)
                </span>
                <span
                  style={{
                    fontFamily: "monospace",
                    fontSize: 28,
                    fontWeight: 700,
                    color: "#ffffff",
                    marginTop: 4,
                  }}
                >
                  {sigma.toFixed(1)}
                </span>
              </div>
            </div>
          </div>
        </div>

        {/* Bottom bar: stats */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            width: "100%",
            borderTop: "1px solid #27272a",
            paddingTop: 20,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 24 }}>
            <div style={{ fontSize: 16, color: "#71717a" }}>relay44.com</div>
            <div
              style={{
                fontSize: 14,
                color: "#52525b",
                letterSpacing: "0.04em",
              }}
            >
              Range: {market.outcomeMin}{unit ? ` ${unit}` : ""} &ndash; {market.outcomeMax}{unit ? ` ${unit}` : ""}
            </div>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 24 }}>
            {collateral !== "$0" ? (
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span style={{ fontSize: 14, color: "#52525b", textTransform: "uppercase", letterSpacing: "0.06em" }}>
                  Collateral
                </span>
                <span style={{ fontFamily: "monospace", fontSize: 16, fontWeight: 600, color: "#a1a1aa" }}>
                  {collateral}
                </span>
              </div>
            ) : null}
            {volume !== "$0" ? (
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span style={{ fontSize: 14, color: "#52525b", textTransform: "uppercase", letterSpacing: "0.06em" }}>
                  Vol
                </span>
                <span style={{ fontFamily: "monospace", fontSize: 16, fontWeight: 600, color: "#a1a1aa" }}>
                  {volume}
                </span>
              </div>
            ) : null}
          </div>
        </div>
      </div>
    ),
    { ...size },
  );
}
