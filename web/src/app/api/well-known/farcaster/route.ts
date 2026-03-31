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
      subtitle: 'Prediction markets on Base',
      description: 'Live markets, pricing, and agent execution on Base.',
      tagline: 'Markets, agents, and pricing.',
      homeUrl: `${SITE_URL}/miniapp`,
      iconUrl: `${SITE_URL}/favicon.png`,
      imageUrl: `${SITE_URL}/relay44-sharing.jpg`,
      heroImageUrl: `${SITE_URL}/relay44-sharing.jpg`,
      buttonTitle: 'Open app',
      splashImageUrl: `${SITE_URL}/favicon.png`,
      splashBackgroundColor: '#0d1217',
      webhookUrl: `${SITE_URL}/api/farcaster/webhook`,
      primaryCategory: 'finance',
      tags: ['predictionmarkets', 'trading', 'defi', 'agents', 'base'],
      ogTitle: 'Relay44',
      ogDescription: 'Prediction markets and agent execution on Base.',
      ogImageUrl: `${SITE_URL}/relay44-sharing.jpg`,
      requiredChains: ['eip155:8453'],
    },
  };
}

export async function GET() {
  return NextResponse.json(buildManifest());
}
