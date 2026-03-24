const ADMIN_WALLETS = new Set(
  (process.env.NEXT_PUBLIC_ADMIN_WALLETS || '')
    .split(',')
    .map((wallet) => wallet.trim().toLowerCase())
    .filter((wallet) => wallet.startsWith('0x') && wallet.length === 42),
);

export function isAdminWallet(address?: string | null) {
  return Boolean(address && ADMIN_WALLETS.has(address.toLowerCase()));
}
