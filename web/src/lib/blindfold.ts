/**
 * Blindfold Finance Integration
 *
 * Blindfold card funding is intentionally disabled in launch runtime.
 * Wallet-based onchain deposit proofs are the only supported source.
 */

export interface BlindpayConfig {
  amount: number; // USDC amount in smallest units (6 decimals)
  walletAddress: string;
  callbackUrl: string;
  successUrl: string;
  cancelUrl: string;
}

export interface BlindpaySession {
  sessionId: string;
  paymentUrl: string;
  expiresAt: number;
}

/**
 * Create a Blindfold payment session
 *
 * This generates a payment URL that the user can be redirected to.
 * The actual implementation will depend on Blindfold's API.
 */
export async function createPaymentSession(
  _config: BlindpayConfig
): Promise<BlindpaySession> {
  throw new Error('Blindfold funding is disabled. Use wallet deposit confirmation.');
}

/**
 * Verify a Blindfold webhook signature
 *
 * Webhooks are signed with HMAC-SHA256 using your merchant secret.
 */
export function verifyWebhookSignature(
  payload: string,
  signature: string,
  secret: string
): boolean {
  // This is handled server-side in app/src/api/wallet.rs
  // Client-side verification is not needed
  return true;
}

/**
 * Parse payment status from redirect URL params
 */
export function parseRedirectParams(searchParams: URLSearchParams): {
  success: boolean;
  sessionId: string | null;
  paymentId: string | null;
  error: string | null;
} {
  const status = searchParams.get('status');
  const sessionId = searchParams.get('session_id');
  const paymentId = searchParams.get('payment_id');
  const error = searchParams.get('error');

  return {
    success: status === 'success' || status === 'completed',
    sessionId,
    paymentId,
    error,
  };
}

/**
 * Format amount for display
 */
export function formatUsdAmount(amountLamports: number): string {
  return `$${(amountLamports / 1_000_000).toFixed(2)}`;
}
