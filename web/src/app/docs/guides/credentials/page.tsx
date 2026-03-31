import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Credentials Guide',
  description: 'Connect external venues to Relay44 — set up Polymarket, Limitless, and Aerodrome credentials for agent execution.',
  path: '/docs/guides/credentials',
  keywords: ['credentials', 'polymarket', 'limitless', 'aerodrome', 'api keys'],
});

const venues = [
  {
    name: 'Polymarket',
    description: 'CLOB-based prediction market on Polygon. Requires an API key and secret from your Polymarket account.',
    steps: ['Log in to Polymarket', 'Go to Settings → API', 'Generate an API key + secret', 'Enter both in Relay44\'s credential form'],
  },
  {
    name: 'Limitless',
    description: 'Prediction market on Base. Uses wallet binding for authentication.',
    steps: ['Navigate to Credentials on Relay44', 'Select Limitless as the provider', 'Complete the wallet bind flow', 'Relay44 manages the session automatically'],
  },
  {
    name: 'Aerodrome',
    description: 'DEX on Base. No API key needed — Relay44 interacts directly with the Aerodrome contracts using your connected wallet.',
    steps: ['Connect your Base wallet to Relay44', 'Aerodrome access is automatic', 'Your wallet signs each swap transaction'],
  },
];

export default function CredentialsGuidePage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/guides/credentials', name: 'Credentials Guide', description: 'Connect external venues to Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Guides', url: '/docs/guides' },
            { name: 'Credentials', url: '/docs/guides/credentials' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Credentials</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Connect external venue credentials so agents can execute trades on your behalf across
        Polymarket, Limitless, and Aerodrome.
      </p>

      <div className="mt-8 grid gap-6">
        {venues.map((venue) => (
          <Card key={venue.name} className="p-6">
            <h2 className="text-lg font-semibold text-text-primary">{venue.name}</h2>
            <p className="mt-2 text-sm leading-6 text-text-secondary">{venue.description}</p>
            <ol className="mt-4 space-y-2">
              {venue.steps.map((step, i) => (
                <li key={i} className="flex items-start gap-3 text-sm text-text-secondary">
                  <span className="flex h-6 w-6 shrink-0 items-center justify-center border border-border text-xs text-text-muted">
                    {i + 1}
                  </span>
                  {step}
                </li>
              ))}
            </ol>
          </Card>
        ))}

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Security</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Credentials are encrypted at rest and never exposed in API responses. Only the owning
            user can use their credentials. You can revoke credentials at any time from the
            Credentials page.
          </p>
        </Card>
      </div>
    </>
  );
}
