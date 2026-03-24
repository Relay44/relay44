import { NextRequest, NextResponse } from 'next/server';

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL || 'https://relay44.com';
const marketUrlPattern = /(?:https?:\/\/)?(?:www\.)?relay44\.com\/markets\/([^\s"?#]+)/;

/** GET: Return action metadata for Farcaster discovery. */
export async function GET() {
  return NextResponse.json({
    name: 'Bet on This',
    icon: 'trending-up',
    description: 'Place a bet on this relay44 market',
    aboutUrl: `${SITE_URL}/about`,
    action: {
      type: 'post',
    },
  });
}

/** POST: Handle the cast action when a user clicks "Bet on This". */
export async function POST(req: NextRequest) {
  try {
    const body = await req.json();
    const embeds = body?.untrustedData?.embeds || [];
    const castText = body?.untrustedData?.text || '';

    // Look for a relay44 market URL in embeds or cast text
    let marketId: string | null = null;

    for (const embed of embeds) {
      const match = (embed?.url || '').match(marketUrlPattern);
      if (match) {
        marketId = decodeURIComponent(match[1]);
        break;
      }
    }

    if (!marketId) {
      const textMatch = castText.match(marketUrlPattern);
      if (textMatch) {
        marketId = decodeURIComponent(textMatch[1]);
      }
    }

    if (!marketId) {
      return NextResponse.json({
        message: 'No relay44 market found in this cast.',
      });
    }

    return NextResponse.json({
      type: 'frame',
      frameUrl: `${SITE_URL}/miniapp/market/${encodeURIComponent(marketId)}`,
    });
  } catch {
    return NextResponse.json(
      { message: 'Failed to process action.' },
      { status: 500 },
    );
  }
}
