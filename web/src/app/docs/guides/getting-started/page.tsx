import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Getting Started',
  description: 'Get started with Relay44 — connect your wallet, browse prediction markets, and place your first trade.',
  path: '/docs/guides/getting-started',
  keywords: ['getting started', 'first trade', 'connect wallet', 'tutorial'],
});

const steps = [
  {
    step: '1',
    title: 'Connect your wallet',
    content: 'Click the connect button in the top-right corner. Relay44 supports EVM wallets (MetaMask, Coinbase Wallet, WalletConnect), Solana wallets, and Farcaster accounts. You\'ll sign a message to prove ownership — no transaction fees for signing in.',
  },
  {
    step: '2',
    title: 'Browse markets',
    content: 'The Markets page shows all available prediction markets across internal and external venues. Use filters to narrow by category, source, or status. Each market shows the current yes/no prices, volume, and time remaining.',
  },
  {
    step: '3',
    title: 'Understand pricing',
    content: 'Prediction market prices represent probabilities. A "Yes" share at $0.65 means the market thinks there\'s a 65% chance the event happens. Yes + No prices always sum to approximately $1.00. You profit when you buy shares below their true probability.',
  },
  {
    step: '4',
    title: 'Place your first order',
    content: 'Select a market, choose an outcome (Yes or No), set your price and quantity, then submit. Your order goes into the order book. When it matches with a counterparty, you receive outcome shares. The transaction is prepared for signing — you confirm in your wallet.',
  },
  {
    step: '5',
    title: 'Track your positions',
    content: 'The Portfolio page shows all your open positions with current value and P&L. When a market resolves, winning positions can be claimed for the payout amount.',
  },
];

export default function GettingStartedPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/guides/getting-started', name: 'Getting Started', description: 'Get started with Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Guides', url: '/docs/guides' },
            { name: 'Getting Started', url: '/docs/guides/getting-started' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Getting Started</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        This guide walks you through connecting your wallet, browsing markets, and placing your
        first trade on Relay44.
      </p>

      <div className="mt-8 grid gap-4">
        {steps.map((s) => (
          <Card key={s.step} className="p-6">
            <div className="flex items-start gap-4">
              <span className="flex h-8 w-8 shrink-0 items-center justify-center border border-border text-sm font-medium text-text-primary">
                {s.step}
              </span>
              <div>
                <h2 className="text-lg font-semibold text-text-primary">{s.title}</h2>
                <p className="mt-2 text-sm leading-6 text-text-secondary">{s.content}</p>
              </div>
            </div>
          </Card>
        ))}
      </div>
    </>
  );
}
