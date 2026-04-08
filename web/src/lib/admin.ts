import { checkAdminWallet } from '@/lib/server/adminGate';

/**
 * Validate admin status via server action.
 * The wallet list is kept server-side so it never leaks into the client bundle.
 */
export async function isAdminWallet(address?: string | null): Promise<boolean> {
  return checkAdminWallet(address ?? null);
}
