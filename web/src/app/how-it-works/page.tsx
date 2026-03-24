import Link from "next/link";
import { PageShell } from "@/components/layout";
import { StructuredData } from "@/components/seo/StructuredData";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/Card";
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from "@/lib/seo";

export const metadata = buildPageMetadata({
  title: "How It Works",
  description:
    "Learn how relay44 markets resolve, what users need to trade or create, what is live now, and where to review legal and support resources.",
  path: "/how-it-works",
  keywords: [
    "how it works",
    "market resolution",
    "launch guide",
    "prediction market FAQ",
  ],
});

const faqItems = [
  {
    question: "Do I need a wallet to use Relay44?",
    answer:
      "You can browse markets without a wallet. You need a Base wallet and an active sign-in session before you can trade, fund, withdraw, or publish a market.",
  },
  {
    question: "How does a market resolve?",
    answer:
      "Every market should resolve to one objective yes or no outcome. The resolution source is declared before publish, trading ends at a fixed UTC deadline, and settlement follows the posted source.",
  },
  {
    question: "What makes a good market question?",
    answer:
      "Good questions are narrow, dated, and auditable. They avoid bundled outcomes, subjective language, and wording that conflicts with the listed source or deadline.",
  },
  {
    question: "Why might trading or creation be unavailable?",
    answer:
      "Write flows can be blocked by preview mode, missing wallet sign-in, unsupported network, insufficient balance, route-level maintenance, or compliance checks.",
  },
  {
    question: "Where do I review legal or support information?",
    answer:
      "Use the legal section for terms, privacy, and risk disclosures. For public support and operational updates, use the linked support and announcement channels.",
  },
];

const supportHref = "mailto:support@relay44.com";

export default function HowItWorksPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: "/how-it-works",
            name: "How Relay44 works",
            description:
              "Launch guidance for market resolution, wallet setup, live trading readiness, and public support resources.",
          }),
          buildBreadcrumbStructuredData([
            { name: "Home", url: "/" },
            { name: "How It Works", url: "/how-it-works" },
          ]),
          {
            "@context": "https://schema.org",
            "@type": "FAQPage",
            mainEntity: faqItems.map((item) => ({
              "@type": "Question",
              name: item.question,
              acceptedAnswer: {
                "@type": "Answer",
                text: item.answer,
              },
            })),
          },
        ]}
      />
      <PageShell>
        <div className="mx-auto max-w-5xl px-4 py-8">
          <section className="border border-border bg-bg-primary p-6 brutal-shadow">
            <p className="text-[11px] uppercase tracking-[0.18em] text-accent">
              Launch primer
            </p>
            <h1 className="mt-3 text-3xl font-semibold uppercase tracking-[-0.04em] text-text-primary sm:text-4xl">
              How Relay44 works
            </h1>
            <p className="mt-4 max-w-3xl text-sm leading-7 text-text-secondary">
              Relay44 is live prediction market infrastructure on Base.
              Public users can browse live markets and order books immediately,
              while trading and market creation depend on wallet sign-in,
              available balance, route state, and the exact market rules shown
              before publish.
            </p>
            <div className="mt-6 flex flex-wrap gap-3">
              <Link
                href="/markets"
                className="inline-flex h-10 items-center border border-accent px-4 text-sm uppercase tracking-[0.12em] text-accent transition-colors hover:bg-accent/10"
              >
                Browse markets
              </Link>
              <Link
                href="/markets/create"
                className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
              >
                Create a market
              </Link>
              <Link
                href="/legal/disclaimer"
                className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
              >
                Risk disclaimer
              </Link>
            </div>
          </section>

          <section className="mt-6 grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle>What is live now</CardTitle>
                <CardDescription>
                  Public launch behavior that users can rely on immediately.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-3 text-sm leading-6 text-text-secondary">
                <p>
                  Browse live markets, odds, order books, and shareable market
                  pages.
                </p>
                <p>
                  Inspect portfolio and wallet state after wallet sign-in is
                  complete.
                </p>
                <p>
                  Draft markets from the live news desk and publish when write
                  rails are available.
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>How markets resolve</CardTitle>
                <CardDescription>
                  Resolution quality is the core trust surface.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-3 text-sm leading-6 text-text-secondary">
                <p>
                  Every market should resolve to one objective yes or no
                  outcome.
                </p>
                <p>
                  The source used for resolution is chosen before publish and
                  should be auditable.
                </p>
                <p>
                  Trading ends at a fixed UTC time, then the market moves into
                  resolution and payout.
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>What you need to trade or create</CardTitle>
                <CardDescription>
                  Live write flows depend on setup, not just page access.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-3 text-sm leading-6 text-text-secondary">
                <p>A Base wallet connected from the header.</p>
                <p>
                  An active sign-in session and enough balance for the intended
                  action.
                </p>
                <p>
                  For market creation, a clean question, a resolution source,
                  and a deadline that matches the wording.
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>What can still be gated</CardTitle>
                <CardDescription>
                  Public launch does not mean every write path is always open.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-3 text-sm leading-6 text-text-secondary">
                <p>
                  Trading and creation can be blocked by runtime mode, route
                  maintenance, or failed sign-in.
                </p>
                <p>
                  Network configuration, wallet state, and compliance checks can
                  also pause write actions.
                </p>
                <p>
                  If a write path is unavailable, read-only market inspection
                  should remain usable.
                </p>
              </CardContent>
            </Card>
          </section>

          <section className="mt-6 grid gap-4 md:grid-cols-3">
            <Card>
              <CardHeader>
                <CardTitle>Legal and risk</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3 text-sm text-text-secondary">
                <Link
                  href="/legal"
                  className="block text-accent transition-colors hover:text-accent-hover"
                >
                  Review legal documents
                </Link>
                <Link
                  href="/legal/disclaimer"
                  className="block text-accent transition-colors hover:text-accent-hover"
                >
                  Read the risk disclaimer
                </Link>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Support</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3 text-sm text-text-secondary">
                <a
                  href={supportHref}
                  target="_blank"
                  rel="noreferrer"
                  className="block text-accent transition-colors hover:text-accent-hover"
                >
                  Public support guide
                </a>
                <a
                  href="https://x.com/relay44"
                  target="_blank"
                  rel="noreferrer"
                  className="block text-accent transition-colors hover:text-accent-hover"
                >
                  Launch announcements
                </a>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Start here</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3 text-sm text-text-secondary">
                <Link
                  href="/markets"
                  className="block text-accent transition-colors hover:text-accent-hover"
                >
                  Browse live markets
                </Link>
                <Link
                  href="/wallet"
                  className="block text-accent transition-colors hover:text-accent-hover"
                >
                  Prepare wallet access
                </Link>
              </CardContent>
            </Card>
          </section>

          <section className="mt-6">
            <div className="mb-4">
              <p className="text-[11px] uppercase tracking-[0.18em] text-text-muted">
                FAQ
              </p>
              <h2 className="mt-2 text-2xl font-semibold uppercase tracking-[-0.03em] text-text-primary">
                Common launch questions
              </h2>
            </div>
            <div className="grid gap-4">
              {faqItems.map((item) => (
                <Card key={item.question}>
                  <CardHeader>
                    <CardTitle>{item.question}</CardTitle>
                  </CardHeader>
                  <CardContent>
                    <p className="text-sm leading-7 text-text-secondary">
                      {item.answer}
                    </p>
                  </CardContent>
                </Card>
              ))}
            </div>
          </section>
        </div>
      </PageShell>
    </>
  );
}
