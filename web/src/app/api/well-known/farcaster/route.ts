import { NextResponse } from 'next/server';

const SITE_URL = (process.env.NEXT_PUBLIC_SITE_URL || 'https://relay44.com').replace(/\/$/, '');

function buildManifest() {
  const header = process.env.FARCASTER_ACCOUNT_ASSOCIATION_HEADER?.trim();
  const payload = process.env.FARCASTER_ACCOUNT_ASSOCIATION_PAYLOAD?.trim();
  const signature = process.env.FARCASTER_ACCOUNT_ASSOCIATION_SIGNATURE?.trim();
  const accountAssociation =
    header && payload && signature
      ? {
          header,
          payload,
          signature,
        }
      : undefined;

  return {
    ...(accountAssociation ? { accountAssociation } : {}),
    miniapp: {
      version: '1',
      name: 'Relay44',
      subtitle: 'Agentic prediction markets',
      description: 'Live prediction markets, decision cells, and automated agent execution on Base.',
      tagline: 'Predict. Trade. Automate.',
      homeUrl: `${SITE_URL}/miniapp`,
      iconUrl: `${SITE_URL}/favicon.png`,
      imageUrl: `${SITE_URL}/og-miniapp-relay44.svg`,
      heroImageUrl: `${SITE_URL}/og-miniapp-relay44.svg`,
      buttonTitle: 'Open Relay44',
      splashImageUrl: `${SITE_URL}/favicon.png`,
      splashBackgroundColor: '#0d1217',
      webhookUrl: `${SITE_URL}/api/farcaster/webhook`,
      primaryCategory: 'finance',
      tags: ['predictionmarkets', 'trading', 'defi', 'agents', 'base'],
      ogTitle: 'Relay44',
      ogDescription: 'Agentic prediction markets and decision automation on Base.',
      ogImageUrl: `${SITE_URL}/og-miniapp-relay44.svg`,
      requiredChains: ['eip155:8453'],
    },
  };
}

export async function GET() {
  return NextResponse.json(buildManifest());
}
