import { NextRequest, NextResponse } from 'next/server';

export async function POST(req: NextRequest) {
  try {
    const body = await req.json();
    const { event, fid, notificationDetails } = body;

    switch (event) {
      case 'frame_added': {
        if (notificationDetails?.token && notificationDetails?.url) {
          console.log(`[farcaster] frame_added: fid=${fid}, notifications enabled`);
        } else {
          console.log(`[farcaster] frame_added: fid=${fid}, no notifications`);
        }
        break;
      }

      case 'frame_removed': {
        console.log(`[farcaster] frame_removed: fid=${fid}`);
        break;
      }

      case 'notifications_enabled': {
        if (notificationDetails?.token && notificationDetails?.url) {
          console.log(`[farcaster] notifications_enabled: fid=${fid}`);
        }
        break;
      }

      case 'notifications_disabled': {
        console.log(`[farcaster] notifications_disabled: fid=${fid}`);
        break;
      }

      default:
        console.log(`[farcaster] unknown event: ${event}`);
    }

    return NextResponse.json({ success: true });
  } catch (err) {
    console.error('[farcaster] webhook error:', err);
    return NextResponse.json({ success: false }, { status: 500 });
  }
}
