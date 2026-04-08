'use server';

const ADMIN_WALLETS = new Set(
  (process.env.ADMIN_WALLETS || process.env.NEXT_PUBLIC_ADMIN_WALLETS || '')
    .split(',')
    .map((wallet) => wallet.trim().toLowerCase())
    .filter((wallet) => wallet.startsWith('0x') && wallet.length === 42),
);

export async function checkAdminWallet(address: string | null | undefined): Promise<boolean> {
  if (!address) return false;
  return ADMIN_WALLETS.has(address.toLowerCase());
}
