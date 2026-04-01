import { ImageResponse } from "next/og";

export const runtime = "edge";

export const size = { width: 1200, height: 630 };

export const contentType = "image/png";

const API_BASE =
  process.env.NEXT_PUBLIC_API_URL?.trim() ||
  "https://relay44-api.onrender.com";

interface MarketData {
  question: string;
  yesPrice: number;
  noPrice: number;
  category: string;
  source: string;
  provider: string;
  status: string;
  imageUrl?: string;
}

export default async function Image({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  let market: MarketData | null = null;

  try {
    const res = await fetch(`${API_BASE}/v1/evm/markets/${id}`, {
      next: { revalidate: 300 },
    });
    if (res.ok) {
      const json = await res.json();
      market = json.data ?? json;
    }
  } catch {
    // fall through to fallback
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
            backgroundColor: "#0a0a0a",
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
          <div
            style={{
              fontSize: 32,
              color: "#a1a1aa",
              textAlign: "center",
            }}
          >
            Market not found
          </div>
        </div>
      ),
      { ...size },
    );
  }

  const yesPercent = Math.round(market.yesPrice * 100);
  const noPercent = Math.round(market.noPrice * 100);
  const hasOutcomePrices = market.yesPrice > 0 || market.noPrice > 0;

  const sourceName =
    market.provider === "polymarket"
      ? "Polymarket"
      : market.provider === "gamma"
        ? "Gamma"
        : market.source || market.provider || "";

  return new ImageResponse(
    (
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          width: "100%",
          height: "100%",
          backgroundColor: "#0a0a0a",
          padding: "48px 60px",
          position: "relative",
        }}
      >
        {/* Top bar: branding + category */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            width: "100%",
            marginBottom: 40,
          }}
        >
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
          {market.category ? (
            <div
              style={{
                display: "flex",
                fontSize: 18,
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

        {/* Question text */}
        <div
          style={{
            display: "flex",
            flex: 1,
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <div
            style={{
              fontSize: market.question.length > 100 ? 36 : 44,
              fontWeight: 600,
              color: "#ffffff",
              lineHeight: 1.3,
              textAlign: "center",
              maxWidth: "1000px",
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

        {/* Probability bars */}
        {hasOutcomePrices ? (
          <div
            style={{
              display: "flex",
              gap: 16,
              width: "100%",
              marginBottom: 32,
            }}
          >
            {/* YES bar */}
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                flex: 1,
              }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  marginBottom: 8,
                }}
              >
                <span
                  style={{ fontSize: 20, fontWeight: 600, color: "#22c55e" }}
                >
                  YES
                </span>
                <span
                  style={{ fontSize: 20, fontWeight: 700, color: "#22c55e" }}
                >
                  {yesPercent}%
                </span>
              </div>
              <div
                style={{
                  display: "flex",
                  width: "100%",
                  height: 12,
                  backgroundColor: "#1a1a1a",
                  borderRadius: 6,
                  overflow: "hidden",
                }}
              >
                <div
                  style={{
                    width: `${yesPercent}%`,
                    height: "100%",
                    backgroundColor: "#22c55e",
                    borderRadius: 6,
                  }}
                />
              </div>
            </div>

            {/* NO bar */}
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                flex: 1,
              }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  marginBottom: 8,
                }}
              >
                <span
                  style={{ fontSize: 20, fontWeight: 600, color: "#ef4444" }}
                >
                  NO
                </span>
                <span
                  style={{ fontSize: 20, fontWeight: 700, color: "#ef4444" }}
                >
                  {noPercent}%
                </span>
              </div>
              <div
                style={{
                  display: "flex",
                  width: "100%",
                  height: 12,
                  backgroundColor: "#1a1a1a",
                  borderRadius: 6,
                  overflow: "hidden",
                }}
              >
                <div
                  style={{
                    width: `${noPercent}%`,
                    height: "100%",
                    backgroundColor: "#ef4444",
                    borderRadius: 6,
                  }}
                />
              </div>
            </div>
          </div>
        ) : null}

        {/* Bottom bar */}
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
          <div style={{ fontSize: 18, color: "#71717a" }}>relay44.com</div>
          {sourceName ? (
            <div style={{ fontSize: 18, color: "#71717a" }}>
              via {sourceName}
            </div>
          ) : null}
        </div>
      </div>
    ),
    { ...size },
  );
}
