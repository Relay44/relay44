const PALETTES = [
  ["#030303", "#f2f2f2", "#4f7cff", "#ed8760", "#22c55e"],
  ["#0a0a0a", "#f2f2f2", "#3b82f6", "#f59e0b", "#16a34a"],
  ["#171717", "#fafafa", "#60a5fa", "#ff8b5f", "#22c55e"],
];

const cache = new Map<string, string>();

function hashSeed(input: string): number {
  let hash = 2166136261;
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function createRandom(seed: number) {
  let value = seed || 1;
  return () => {
    value = (value * 1664525 + 1013904223) >>> 0;
    return value / 0xffffffff;
  };
}

export function buildMarketArtworkDataUrl(seedText: string): string {
  const cacheKey = seedText.trim() || "relay44";
  const cached = cache.get(cacheKey);
  if (cached) {
    return cached;
  }

  const seed = hashSeed(cacheKey);
  const random = createRandom(seed);
  const palette = PALETTES[seed % PALETTES.length];
  const [background, foreground, accentA, accentB, accentC] = palette;
  const colors = [foreground, accentA, accentB, accentC];
  const gridSize = 12;
  const cellSize = 12;
  const width = gridSize * cellSize;
  const height = gridSize * cellSize;
  const midPoint = Math.ceil(gridSize / 2);
  const blocks: string[] = [];

  for (let y = 0; y < gridSize; y += 1) {
    for (let x = 0; x < midPoint; x += 1) {
      if (random() < 0.44) {
        continue;
      }

      const color = colors[Math.floor(random() * colors.length)] || foreground;
      const mirrorX = gridSize - x - 1;
      const rectY = y * cellSize;
      const rectX = x * cellSize;
      blocks.push(
        `<rect x="${rectX}" y="${rectY}" width="${cellSize}" height="${cellSize}" fill="${color}" />`
      );
      if (mirrorX !== x) {
        blocks.push(
          `<rect x="${mirrorX * cellSize}" y="${rectY}" width="${cellSize}" height="${cellSize}" fill="${color}" />`
        );
      }
    }
  }

  const svg = `
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}" shape-rendering="crispEdges">
      <rect width="${width}" height="${height}" fill="${background}" />
      <rect x="6" y="6" width="${width - 12}" height="${height - 12}" fill="none" stroke="${foreground}" stroke-opacity="0.18" stroke-width="2" />
      <rect x="0" y="${height - 24}" width="${width}" height="24" fill="${background}" fill-opacity="0.88" />
      ${blocks.join("")}
      <text x="14" y="${height - 8}" fill="${foreground}" fill-opacity="0.82" font-family="monospace" font-size="12" font-weight="700">01</text>
      <text x="${width - 34}" y="20" fill="${foreground}" fill-opacity="0.68" font-family="monospace" font-size="10" font-weight="700">10</text>
    </svg>
  `;

  const encoded = `data:image/svg+xml;charset=UTF-8,${encodeURIComponent(svg)}`;
  cache.set(cacheKey, encoded);
  return encoded;
}
