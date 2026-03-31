import { StructuredData } from '@/components/seo/StructuredData';
import { EndpointGroup, type Endpoint } from '@/components/docs';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Auth API',
  description: 'Authentication endpoints for Relay44 — SIWE, Solana, Farcaster login flows, JWT refresh, and session management.',
  path: '/docs/api/auth',
  keywords: ['authentication', 'siwe', 'farcaster', 'solana', 'jwt', 'login'],
});

const groups: Array<{ title: string; description: string; endpoints: Endpoint[] }> = [
  {
    title: 'Generic auth',
    description: 'Chain-agnostic nonce and login endpoints.',
    endpoints: [
      { method: 'GET', path: '/v1/auth/nonce', description: 'Generate a challenge nonce for any auth flow' },
      { method: 'POST', path: '/v1/auth/login', description: 'Verify a signed message and issue JWT' },
    ],
  },
  {
    title: 'SIWE (Sign-In with Ethereum)',
    description: 'EVM wallet authentication using EIP-4361.',
    endpoints: [
      { method: 'GET', path: '/v1/auth/siwe/nonce', description: 'Generate SIWE-specific nonce' },
      { method: 'POST', path: '/v1/auth/siwe/login', description: 'Verify SIWE signature, return JWT + refresh token' },
    ],
  },
  {
    title: 'Solana',
    description: 'Solana wallet sign-in.',
    endpoints: [
      { method: 'GET', path: '/v1/auth/solana/nonce', description: 'Generate Solana nonce' },
      { method: 'POST', path: '/v1/auth/solana/login', description: 'Verify Solana signature, return JWT' },
    ],
  },
  {
    title: 'Farcaster',
    description: 'Farcaster social login via Neynar.',
    endpoints: [
      { method: 'GET', path: '/v1/auth/farcaster/nonce', description: 'Generate Farcaster nonce' },
      { method: 'POST', path: '/v1/auth/farcaster/login', description: 'Verify Farcaster custody address, return JWT' },
    ],
  },
  {
    title: 'Session management',
    description: 'Refresh and revoke tokens.',
    endpoints: [
      { method: 'POST', path: '/v1/auth/refresh', description: 'Refresh an expired access token using the refresh token', auth: true },
      { method: 'POST', path: '/v1/auth/logout', description: 'Revoke the current session', auth: true },
    ],
  },
];

export default function AuthApiPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/api/auth', name: 'Auth API', description: 'Authentication endpoints for Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'API', url: '/docs/api' },
            { name: 'Auth', url: '/docs/api/auth' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Auth API</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Relay44 supports multi-chain authentication. Users can sign in with an EVM wallet (SIWE),
        Solana wallet, or Farcaster account. All flows produce a JWT access token and a refresh token.
      </p>

      <div className="mt-8 grid gap-4">
        {groups.map((g) => (
          <EndpointGroup key={g.title} {...g} />
        ))}
      </div>
    </>
  );
}
