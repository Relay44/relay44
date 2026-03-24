import { NextRequest, NextResponse } from 'next/server';

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL || 'https://relay44.com';

/** GET: Return action metadata for Farcaster discovery. */
export async function GET() {
  return NextResponse.json({
    name: 'Create Market',
    icon: 'plus-circle',
    description: 'Create a prediction market about this cast on relay44',
    aboutUrl: `${SITE_URL}/about`,
    action: {
      type: 'post',
    },
  });
}

/** POST: Handle the cast action when a user clicks "Create Market". */
export async function POST(req: NextRequest) {
  try {
    const body = await req.json();
    const castText = body?.untrustedData?.text || '';

    // Truncate to reasonable length for a market question
    const question = castText.slice(0, 200).trim();

    if (!question) {
      return NextResponse.json(
        { message: 'Cast has no text to create a market from.' },
        { status: 400 },
      );
    }

    // Return a frame URL that opens the Mini App with the question pre-filled
    const encodedQuestion = encodeURIComponent(question);
    return NextResponse.json({
      type: 'frame',
      frameUrl: `${SITE_URL}/miniapp?create=true&question=${encodedQuestion}`,
    });
  } catch {
    return NextResponse.json(
      { message: 'Failed to process action.' },
      { status: 500 },
    );
  }
}
