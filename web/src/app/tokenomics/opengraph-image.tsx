import { ImageResponse } from "next/og";

export const runtime = "edge";

export const alt = "Relay44 Tokenomics — $RELAY supply, fee flow, and staking tiers";

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
            Tokenomics
          </div>
        </div>

        {/* Headline */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            marginTop: 72,
          }}
        >
          <div
            style={{
              fontSize: 96,
              fontWeight: 700,
              color: "#ffffff",
              lineHeight: 1,
              letterSpacing: "-0.03em",
            }}
          >
            $RELAY
          </div>
          <div
            style={{
              fontSize: 42,
              fontWeight: 600,
              color: "#ffffff",
              lineHeight: 1.15,
              letterSpacing: "-0.01em",
              marginTop: 18,
              maxWidth: 960,
            }}
          >
            Fees capture value.
            <br />
            Stakers earn the flow.
          </div>
        </div>

        {/* Stat grid */}
        <div
          style={{
            display: "flex",
            gap: 16,
            marginTop: "auto",
            paddingTop: 40,
          }}
        >
          <Stat label="Total supply" value="1B" suffix="$RELAY" />
          <Stat label="Fee routing" value="100%" suffix="to stakers" />
          <Stat label="Tiers" value="4" suffix="staking tiers" />
          <Stat label="Chain" value="BASE" suffix="mainnet" />
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
            marginTop: 32,
          }}
        >
          <div style={{ fontSize: 16, color: "#71717a" }}>
            relay44.com/tokenomics
          </div>
          <div
            style={{
              fontSize: 14,
              color: "#52525b",
              textTransform: "uppercase",
              letterSpacing: "0.14em",
            }}
          >
            Prediction Markets on Base
          </div>
        </div>
      </div>
    ),
    { ...size },
  );
}

function Stat({
  label,
  value,
  suffix,
}: {
  label: string;
  value: string;
  suffix: string;
}) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        flex: 1,
        border: "1px solid #27272a",
        padding: "18px 20px",
      }}
    >
      <div
        style={{
          fontSize: 12,
          color: "#71717a",
          textTransform: "uppercase",
          letterSpacing: "0.14em",
        }}
      >
        {label}
      </div>
      <div
        style={{
          display: "flex",
          alignItems: "baseline",
          gap: 8,
          marginTop: 10,
        }}
      >
        <span
          style={{
            fontFamily: "monospace",
            fontSize: 34,
            fontWeight: 700,
            color: "#ffffff",
            letterSpacing: "-0.02em",
          }}
        >
          {value}
        </span>
        <span style={{ fontSize: 13, color: "#71717a" }}>{suffix}</span>
      </div>
    </div>
  );
}
