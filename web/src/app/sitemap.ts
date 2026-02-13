import type { MetadataRoute } from "next";
import { SITE_URL } from "@/lib/seo";
import { fetchAllSeoMarkets, fetchSeoLeaderboard } from "@/lib/server/seo";

export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  const now = new Date();
  const [markets, leaderboardEntries] = await Promise.all([
    fetchAllSeoMarkets(100, 10),
    fetchSeoLeaderboard(50),
  ]);

  const staticRoutes: MetadataRoute.Sitemap = [
    {
      url: SITE_URL,
      lastModified: now,
      changeFrequency: "hourly",
      priority: 1,
    },
    {
      url: `${SITE_URL}/markets`,
      lastModified: now,
      changeFrequency: "hourly",
      priority: 0.95,
    },
    {
      url: `${SITE_URL}/how-it-works`,
      lastModified: now,
      changeFrequency: "weekly",
      priority: 0.75,
    },
    {
      url: `${SITE_URL}/agents`,
      lastModified: now,
      changeFrequency: "daily",
      priority: 0.8,
    },
    {
      url: `${SITE_URL}/leaderboard`,
      lastModified: now,
      changeFrequency: "daily",
      priority: 0.8,
    },
    {
      url: `${SITE_URL}/docs/api`,
      lastModified: now,
      changeFrequency: "weekly",
      priority: 0.7,
    },
    {
      url: `${SITE_URL}/legal`,
      lastModified: now,
      changeFrequency: "monthly",
      priority: 0.55,
    },
    {
      url: `${SITE_URL}/legal/terms`,
      lastModified: now,
      changeFrequency: "monthly",
      priority: 0.5,
    },
    {
      url: `${SITE_URL}/legal/privacy`,
      lastModified: now,
      changeFrequency: "monthly",
      priority: 0.5,
    },
    {
      url: `${SITE_URL}/legal/disclaimer`,
      lastModified: now,
      changeFrequency: "monthly",
      priority: 0.5,
    },
  ];

  const marketRoutes: MetadataRoute.Sitemap = markets.map((market) => ({
    url: `${SITE_URL}/markets/${encodeURIComponent(market.id)}`,
    lastModified: new Date(
      market.resolvedAt || market.tradingEnd || market.createdAt || now,
    ),
    changeFrequency: market.status === "active" ? "hourly" : "daily",
    priority: market.status === "active" ? 0.85 : 0.65,
  }));

  const profileRoutes: MetadataRoute.Sitemap = [
    ...new Set(leaderboardEntries.map((entry) => entry.wallet)),
  ]
    .filter(Boolean)
    .map((wallet) => ({
      url: `${SITE_URL}/profile/${wallet}`,
      lastModified: now,
      changeFrequency: "daily",
      priority: 0.65,
    }));

  return [...staticRoutes, ...marketRoutes, ...profileRoutes];
}

