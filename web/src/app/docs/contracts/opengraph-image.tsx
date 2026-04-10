import { ImageResponse } from "next/og";

export const runtime = "edge";

export const alt =
  "Relay44 Protocol Reference — contract addresses, ABIs, and viem snippets";

export const size = { width: 1200, height: 630 };

export const contentType = "image/png";

export default function Image() {
  return new ImageResponse(
    (
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          width: "100%",
          height: "100%",
          backgroundColor: "#030303",
          padding: "56px 64px",
          position: "relative",
        }}
      >
        {/* Top bar: wordmark + section badge */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            width: "100%",
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
          <div
            style={{
              display: "flex",
              fontSize: 14,
              fontWeight: 600,
              color: "#a1a1aa",
              border: "1px solid #27272a",
              padding: "6px 14px",
              borderRadius: 4,
              textTransform: "uppercase",
              letterSpacing: "0.14em",
            }}
          >
            Protocol Reference
          </div>
        </div>

        {/* Headline */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            marginTop: 52,
          }}
        >
          <div
            style={{
              fontSize: 68,
              fontWeight: 700,
              color: "#ffffff",
              lineHeight: 1.05,
              letterSpacing: "-0.03em",
              maxWidth: 980,
            }}
          >
            Contracts, ABIs &amp; viem
          </div>
          <div
            style={{
              fontSize: 24,
              color: "#a1a1aa",
              lineHeight: 1.3,
              marginTop: 14,
              maxWidth: 960,
            }}
          >
            Everything you need to build on the Relay44 Protocol —
            live on Base mainnet.
          </div>
        </div>

        {/* Code block */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            marginTop: 32,
            border: "1px solid #27272a",
            backgroundColor: "#0a0a0a",
            padding: "22px 26px",
            fontFamily: "monospace",
            fontSize: 18,
            lineHeight: 1.55,
            color: "#d4d4d8",
          }}
        >
          <div style={{ display: "flex" }}>
            <span style={{ color: "#71717a" }}>$&nbsp;</span>
            <span style={{ color: "#ffffff" }}>
              curl relay44.com/api/contracts/market-core/abi
            </span>
          </div>
          <div style={{ display: "flex", marginTop: 6 }}>
            <span style={{ color: "#71717a" }}>
              # MarketCore&nbsp;&nbsp;
            </span>
            <span style={{ color: "#a1a1aa" }}>
              0xc9259a18696Ecbf7636C1a01F40Bc9d47e249AE8
            </span>
          </div>
          <div style={{ display: "flex", marginTop: 4 }}>
            <span style={{ color: "#71717a" }}>
              # OrderBook&nbsp;&nbsp;&nbsp;
            </span>
            <span style={{ color: "#a1a1aa" }}>
              0x6F9CA4aAEaC13f22ce5D6b4657b2eE4bDFAc6c60
            </span>
          </div>
        </div>

        {/* Bottom bar */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            width: "100%",
            borderTop: "1px solid #27272a",
            paddingTop: 18,
            marginTop: "auto",
          }}
        >
          <div style={{ fontSize: 16, color: "#71717a" }}>
            relay44.com/docs/contracts
          </div>
          <div
            style={{
              display: "flex",
              gap: 10,
              fontSize: 13,
              color: "#52525b",
              textTransform: "uppercase",
              letterSpacing: "0.14em",
            }}
          >
            <span>MarketCore</span>
            <span>·</span>
            <span>OrderBook</span>
            <span>·</span>
            <span>RelayStaking</span>
            <span>·</span>
            <span>ERC20</span>
          </div>
        </div>
      </div>
    ),
    { ...size },
  );
}
