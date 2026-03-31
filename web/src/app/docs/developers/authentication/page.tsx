import { StructuredData } from '@/components/seo/StructuredData';
import { CodeBlock } from '@/components/docs';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Authentication',
  description: 'Relay44 authentication flows — SIWE, Solana, Farcaster sign-in, JWT lifecycle, and token refresh.',
  path: '/docs/developers/authentication',
  keywords: ['authentication', 'siwe', 'jwt', 'farcaster', 'solana', 'sign-in'],
});

export default function AuthenticationPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/developers/authentication', name: 'Authentication', description: 'Auth flows for Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Developers', url: '/docs/developers' },
            { name: 'Authentication', url: '/docs/developers/authentication' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Authentication</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Relay44 supports three authentication methods. All produce a JWT access token for
        subsequent API calls.
      </p>

      <div className="mt-8 grid gap-6">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">SIWE (Sign-In with Ethereum)</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            The standard flow for EVM wallets. Request a nonce, construct a SIWE message, sign it
            with the wallet, and submit the signature.
          </p>
          <div className="mt-4 space-y-3">
            <CodeBlock language="bash" code="# 1. Get nonce\ncurl https://relay44.com/v1/auth/siwe/nonce" />
            <CodeBlock
              language="bash"
              code={`# 2. Login with signed message\ncurl -X POST https://relay44.com/v1/auth/siwe/login \\
  -H 'Content-Type: application/json' \\
  -d '{"message": "<siwe-message>", "signature": "<0x-signature>"}'`}
            />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Solana</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Same nonce-sign-verify flow but for Solana wallets. The signature uses the wallet&apos;s
            ed25519 key pair.
          </p>
          <div className="mt-4">
            <CodeBlock
              language="bash"
              code={`curl -X POST https://relay44.com/v1/auth/solana/login \\
  -H 'Content-Type: application/json' \\
  -d '{"message": "<nonce>", "signature": "<base58-signature>", "pubkey": "<base58-pubkey>"}'`}
            />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Farcaster</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Authenticate using your Farcaster custody address. Relay44 verifies through Neynar.
          </p>
          <div className="mt-4">
            <CodeBlock
              language="bash"
              code={`curl -X POST https://relay44.com/v1/auth/farcaster/login \\
  -H 'Content-Type: application/json' \\
  -d '{"message": "<nonce>", "signature": "<custody-sig>", "fid": 12345}'`}
            />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">JWT lifecycle</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            On successful login, you receive an <code className="text-text-primary">access_token</code> (short-lived)
            and a <code className="text-text-primary">refresh_token</code> (long-lived). Include the access token in
            all authenticated requests. When it expires, use the refresh endpoint to get a new one.
          </p>
          <div className="mt-4 space-y-3">
            <CodeBlock
              language="bash"
              code={`# Use access token\ncurl -H 'Authorization: Bearer <access_token>' \\
  https://relay44.com/v1/positions`}
            />
            <CodeBlock
              language="bash"
              code={`# Refresh expired token\ncurl -X POST https://relay44.com/v1/auth/refresh \\
  -H 'Content-Type: application/json' \\
  -d '{"refresh_token": "<refresh_token>"}'`}
            />
          </div>
        </Card>
      </div>
    </>
  );
}
