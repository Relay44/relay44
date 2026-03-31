type IconKind =
  | "ballot"
  | "bot"
  | "chart"
  | "cloud"
  | "coin"
  | "court"
  | "globe"
  | "health"
  | "rocket"
  | "signal"
  | "star"
  | "trophy";

type ProviderKind = "internal" | "limitless" | "polymarket" | "aerodrome";

const cache = new Map<string, string>();

const ICON_PATTERNS: Record<IconKind, readonly string[]> = {
  ballot: [
    "............",
    "...++.......",
    "..++##......",
    ".++..##.....",
    ".....##.....",
    "...####.....",
    "..######....",
    "..##..##....",
    "..##..##....",
    "..######....",
    "............",
    "............",
  ],
  bot: [
    "............",
    "...+..+.....",
    "...++++.....",
    "..######....",
    ".##+..+##...",
    ".##.##.##...",
    ".########...",
    ".##+..+##...",
    "..######....",
    "...#..#.....",
    "............",
    "............",
  ],
  chart: [
    "............",
    ".#..........",
    ".##.........",
    ".#..+.......",
    ".#..++......",
    ".#...+......",
    ".#...++.....",
    ".#....+++...",
    ".#......##..",
    ".##########.",
    "............",
    "............",
  ],
  cloud: [
    "............",
    "............",
    "....###.....",
    "...######...",
    "..##+.+###..",
    ".##########.",
    "..########..",
    ".....++.....",
    "....+..+....",
    "............",
    "............",
    "............",
  ],
  coin: [
    "............",
    "....++++....",
    "...+####+...",
    "..+#....#+..",
    "..##.++.##..",
    "..##.+..##..",
    "..##.++.##..",
    "..+#....#+..",
    "...+####+...",
    "....++++....",
    "............",
    "............",
  ],
  court: [
    "............",
    ".....++.....",
    "...++++++...",
    "..########..",
    "...#....#...",
    "...#....#...",
    "..##....##..",
    "..##....##..",
    "..########..",
    ".##......##.",
    "............",
    "............",
  ],
  globe: [
    "............",
    "....++++....",
    "...+####+...",
    "..+#.##.#+..",
    "..##.##.##..",
    "..########..",
    "..##.##.##..",
    "..+#.##.#+..",
    "...+####+...",
    "....++++....",
    "............",
    "............",
  ],
  health: [
    "............",
    ".....++.....",
    ".....++.....",
    ".....++.....",
    "...######...",
    "..##++++##..",
    "...######...",
    ".....++.....",
    ".....++.....",
    ".....++.....",
    "............",
    "............",
  ],
  rocket: [
    ".....++.....",
    "....+##+....",
    "...+####+...",
    "...######...",
    "...######...",
    "....####....",
    "...##++##...",
    "..##....##..",
    "..+......+..",
    "............",
    "............",
    "............",
  ],
  signal: [
    "............",
    "............",
    "..##....++..",
    "..###...##..",
    "..####..##..",
    "..##.##.##..",
    "..##..####..",
    "..##...###..",
    "..++....##..",
    "............",
    "............",
    "............",
  ],
  star: [
    "............",
    ".....++.....",
    "...++##++...",
    "....####....",
    ".##########.",
    "...######...",
    "..##.++.##..",
    ".##......##.",
    "............",
    "............",
    "............",
    "............",
  ],
  trophy: [
    "............",
    "..########..",
    "..##....##..",
    "...######...",
    "....####....",
    "...+####+...",
    ".....##.....",
    "...######...",
    "..##....##..",
    "............",
    "............",
    "............",
  ],
};

const KIND_KEYWORDS: Array<{ kind: IconKind; words: readonly string[] }> = [
  {
    kind: "trophy",
    words: [
      "sport",
      "sports",
      "game",
      "games",
      "team",
      "match",
      "playoff",
      "playoffs",
      "final",
      "finals",
      "championship",
      "cup",
      "league",
      "nba",
      "nfl",
      "mlb",
      "nhl",
      "ufc",
      "fifa",
      "soccer",
      "football",
      "baseball",
      "basketball",
      "tennis",
      "golf",
    ],
  },
  {
    kind: "ballot",
    words: [
      "election",
      "elections",
      "vote",
      "voter",
      "president",
      "prime minister",
      "senate",
      "house",
      "governor",
      "mayor",
      "parliament",
      "campaign",
      "democrat",
      "republican",
      "poll",
      "polls",
    ],
  },
  {
    kind: "cloud",
    words: [
      "weather",
      "rain",
      "snow",
      "storm",
      "hurricane",
      "wind",
      "temperature",
      "temp",
      "climate",
      "heat",
      "cold",
      "sunny",
      "forecast",
    ],
  },
  {
    kind: "bot",
    words: [
      "ai",
      "openai",
      "gpt",
      "llm",
      "model",
      "models",
      "agent",
      "agents",
      "robot",
      "bot",
      "automation",
      "chip",
      "nvidia",
    ],
  },
  {
    kind: "rocket",
    words: [
      "space",
      "spacex",
      "nasa",
      "launch",
      "moon",
      "mars",
      "rocket",
      "satellite",
      "orbit",
    ],
  },
  {
    kind: "health",
    words: [
      "health",
      "medical",
      "medicine",
      "drug",
      "vaccine",
      "fda",
      "covid",
      "disease",
      "hospital",
      "trial",
      "therapy",
    ],
  },
  {
    kind: "court",
    words: [
      "court",
      "lawsuit",
      "legal",
      "law",
      "judge",
      "sec",
      "regulation",
      "regulator",
      "ban",
      "tariff",
      "policy",
    ],
  },
  {
    kind: "star",
    words: [
      "movie",
      "film",
      "music",
      "album",
      "song",
      "artist",
      "actor",
      "celebrity",
      "grammy",
      "oscar",
      "show",
      "stream",
    ],
  },
  {
    kind: "globe",
    words: [
      "world",
      "global",
      "country",
      "war",
      "china",
      "usa",
      "us ",
      "europe",
      "eu",
      "russia",
      "ukraine",
      "middle east",
      "geopolit",
      "international",
    ],
  },
  {
    kind: "coin",
    words: [
      "bitcoin",
      "btc",
      "ethereum",
      "eth",
      "solana",
      "sol",
      "xrp",
      "doge",
      "token",
      "coin",
      "crypto",
      "memecoin",
      "base",
    ],
  },
  {
    kind: "chart",
    words: [
      "stock",
      "stocks",
      "price",
      "trading",
      "market",
      "earnings",
      "revenue",
      "shares",
      "ipo",
      "fed",
      "inflation",
      "cpi",
      "rates",
      "nasdaq",
      "s&p",
      "spx",
      "tradingview",
    ],
  },
];

const CATEGORY_KINDS: Array<{ kind: IconKind; words: readonly string[] }> = [
  { kind: "coin", words: ["crypto", "token", "coins"] },
  { kind: "trophy", words: ["sports", "sport"] },
  { kind: "ballot", words: ["politics", "election"] },
  { kind: "cloud", words: ["weather", "climate"] },
  { kind: "bot", words: ["tech", "technology", "ai"] },
  { kind: "health", words: ["health", "science"] },
  { kind: "globe", words: ["world", "news"] },
  { kind: "chart", words: ["finance", "business", "economy"] },
];

const ICON_ACCENTS: Record<IconKind, string> = {
  ballot: "#ed8760",
  bot: "#4f7cff",
  chart: "#4f7cff",
  cloud: "#75d1ff",
  coin: "#f59e0b",
  court: "#88a7ff",
  globe: "#22c55e",
  health: "#ef5c56",
  rocket: "#ed8760",
  signal: "#4f7cff",
  star: "#f59e0b",
  trophy: "#22c55e",
};

function hashSeed(input: string): number {
  let hash = 2166136261;
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function normalizeText(input: string): string {
  return input.toLowerCase().replace(/[^a-z0-9\s]/g, " ");
}

function detectIconKind(seedText: string): IconKind {
  const normalized = normalizeText(seedText);
  let bestKind: IconKind = "signal";
  let bestScore = 0;

  for (const rule of CATEGORY_KINDS) {
    for (const word of rule.words) {
      if (!normalized.includes(word)) {
        continue;
      }
      const score = 3;
      if (score > bestScore) {
        bestKind = rule.kind;
        bestScore = score;
      }
    }
  }

  for (const rule of KIND_KEYWORDS) {
    let score = 0;
    for (const word of rule.words) {
      if (normalized.includes(word)) {
        score += word.length > 5 ? 2 : 1;
      }
    }
    if (score > bestScore) {
      bestKind = rule.kind;
      bestScore = score;
    }
  }

  return bestKind;
}

function detectProvider(seedText: string): ProviderKind {
  const normalized = normalizeText(seedText);
  if (normalized.includes("polymarket")) {
    return "polymarket";
  }
  if (normalized.includes("limitless")) {
    return "limitless";
  }
  return "internal";
}

function renderPattern(
  pattern: readonly string[],
  xOffset: number,
  yOffset: number,
  cellSize: number,
  foreground: string,
  accent: string
): string {
  const pixels: string[] = [];

  for (let row = 0; row < pattern.length; row += 1) {
    for (let column = 0; column < pattern[row].length; column += 1) {
      const cell = pattern[row][column];
      if (cell !== "#" && cell !== "+") {
        continue;
      }

      pixels.push(
        `<rect x="${xOffset + column * cellSize}" y="${yOffset + row * cellSize}" width="${cellSize}" height="${cellSize}" fill="${cell === "+" ? accent : foreground}" />`
      );
    }
  }

  return pixels.join("");
}

function renderProviderMarker(
  provider: ProviderKind,
  size: number,
  accent: string,
  foreground: string
): string {
  if (provider === "limitless") {
    return `
      <rect x="${size - 24}" y="14" width="10" height="4" fill="${accent}" />
      <rect x="${size - 24}" y="22" width="16" height="4" fill="${foreground}" fill-opacity="0.55" />
    `;
  }

  if (provider === "polymarket") {
    return `
      <rect x="${size - 24}" y="14" width="4" height="12" fill="${accent}" />
      <rect x="${size - 16}" y="14" width="4" height="12" fill="${foreground}" fill-opacity="0.55" />
    `;
  }

  return `
    <rect x="${size - 24}" y="14" width="12" height="12" fill="none" stroke="${accent}" stroke-width="2" stroke-opacity="0.9" />
  `;
}

export function buildMarketArtworkDataUrl(seedText: string): string {
  const cacheKey = seedText.trim() || "relay44";
  const cached = cache.get(cacheKey);
  if (cached) {
    return cached;
  }

  const seed = hashSeed(cacheKey);
  const iconKind = detectIconKind(cacheKey);
  const providerKind = detectProvider(cacheKey);
  const accent = ICON_ACCENTS[iconKind];
  const background = "#040404";
  const panel = seed % 2 === 0 ? "#090909" : "#0b0b0b";
  const foreground = "#f2f2f2";
  const muted = "#5f5f5f";
  const size = 96;
  const cellSize = 5;
  const pattern = ICON_PATTERNS[iconKind];
  const xOffset = Math.floor((size - pattern[0].length * cellSize) / 2);
  const yOffset = Math.floor((size - pattern.length * cellSize) / 2) + 1;
  const markerOffset = seed % 3;

  const svg = `
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}" shape-rendering="crispEdges">
      <rect width="${size}" height="${size}" fill="${background}" />
      <rect x="8" y="8" width="${size - 16}" height="${size - 16}" fill="${panel}" />
      <rect x="8" y="8" width="${size - 16}" height="${size - 16}" fill="none" stroke="${muted}" stroke-opacity="0.32" stroke-width="2" />
      <rect x="8" y="${size - 14}" width="${size - 16}" height="2" fill="${muted}" fill-opacity="0.26" />
      <rect x="${12 + markerOffset * 4}" y="14" width="14" height="4" fill="${accent}" fill-opacity="0.9" />
      <rect x="${12 + markerOffset * 4}" y="22" width="8" height="4" fill="${foreground}" fill-opacity="0.5" />
      ${renderProviderMarker(providerKind, size, accent, foreground)}
      ${renderPattern(pattern, xOffset, yOffset, cellSize, foreground, accent)}
    </svg>
  `;

  const encoded = `data:image/svg+xml;charset=UTF-8,${encodeURIComponent(svg)}`;
  cache.set(cacheKey, encoded);
  return encoded;
}
