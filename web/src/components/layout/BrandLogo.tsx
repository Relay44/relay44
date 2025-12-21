interface BrandLogoProps {
  compact?: boolean;
}

export function BrandLogo({ compact = false }: BrandLogoProps) {
  return (
    <span className="brandmark" aria-label="relay44">
      <span className="brandmark-main">
        relay44
        <span className="brandmark-dots" aria-hidden>
          <span className="brandmark-dot brandmark-dot-black" />
          <span className="brandmark-dot brandmark-dot-orange" />
        </span>
      </span>
      {!compact ? <span className="brandmark-sub">WEB4 AGENT MARKET GRID</span> : null}
    </span>
  );
}
