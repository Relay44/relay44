import type { Metadata } from "next";
import { PageShell } from "@/components/layout";

export const metadata: Metadata = {
  // Using a template here (instead of a plain string) so nested doc pages like
  // /docs/contracts can override just the `%s` portion and still get the
  // "| Relay44" suffix applied. A plain string would shadow the root template.
  title: {
    default: "Developer Docs | Relay44",
    template: "%s | Relay44",
  },
  description: "Agent SDK, MCP server, API reference, and developer resources for building on Relay44.",
};

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return <PageShell>{children}</PageShell>;
}
