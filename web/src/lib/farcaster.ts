import { sdk } from '@farcaster/miniapp-sdk';
import type { Context } from '@farcaster/miniapp-sdk';

export function isMiniApp(): boolean {
  if (typeof window === 'undefined') return false;
  try {
    return (
      window.parent !== window ||
      !!window.ReactNativeWebView ||
      window.location.search.includes('fc-')
    );
  } catch {
    // Cross-origin frame access throws — means we're in a frame
    return true;
  }
}

export async function ready() {
  try {
    await Promise.race([
      sdk.actions.ready(),
      new Promise<void>((resolve) => setTimeout(resolve, 2000)),
    ]);
  } catch (err) {
    console.warn('Farcaster sdk.actions.ready() failed:', err);
  }
}

export async function close() {
  await sdk.actions.close();
}

export async function addMiniApp() {
  await sdk.actions.addMiniApp();
}

export async function composeCast(options: {
  text?: string;
  embeds?: [] | [string] | [string, string];
}) {
  await sdk.actions.composeCast({
    text: options.text,
    embeds: options.embeds,
  });
}

export async function signInWithFarcaster(nonce: string) {
  return sdk.actions.signIn({ nonce });
}

export async function getEthereumProvider() {
  return sdk.wallet.getEthereumProvider();
}

export async function getContext(): Promise<Context.MiniAppContext | null> {
  try {
    return await Promise.race([
      Promise.resolve(sdk.context),
      new Promise<null>((resolve) => setTimeout(() => resolve(null), 2000)),
    ]);
  } catch {
    return null;
  }
}

export async function viewProfile(fid: number) {
  await sdk.actions.viewProfile({ fid });
}

export async function swapToken(caip19AssetId: string) {
  return sdk.actions.swapToken({ buyToken: caip19AssetId });
}

export { sdk };
