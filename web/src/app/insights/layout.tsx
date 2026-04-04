import { StructuredData } from "@/components/seo/StructuredData";
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from "@/lib/seo";

export const metadata = buildPageMetadata({
  title: "Insights",
  description:
    "Market signals, edge analysis, and research briefs for active prediction markets on Relay44.",
  path: "/insights",
  keywords: [
    "market insights",
    "prediction market analysis",
    "edge signals",
    "research",
  ],
});

export default function InsightsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: "/insights",
            name: "Relay44 market insights",
            description:
              "Market signals, edge analysis, and research briefs for active prediction markets.",
          }),
          buildBreadcrumbStructuredData([
            { name: "Home", url: "/" },
            { name: "Insights", url: "/insights" },
          ]),
        ]}
      />
      {children}
    </>
  );
}
