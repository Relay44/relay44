import type { Metadata } from "next";
import { PageShell } from "@/components/layout";

export const metadata: Metadata = {
  title: "Developer Docs | Relay44",
  description: "Agent SDK, MCP server, API reference, and developer resources for building on Relay44.",
};

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return <PageShell>{children}</PageShell>;
}
