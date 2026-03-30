import { BASE_CHAIN_ID } from '@/lib/constants';

const BASE_MAINNET_CHAIN_ID = 8453;

interface PayResult {
  id: string;
}

interface PaymentStatus {
  status: 'pending' | 'completed' | 'failed';
  sender?: string;
  amount?: string;
  recipient?: string;
}

export async function payWithBase(amount: string, to: string): Promise<PayResult> {
  if (typeof window === 'undefined' || !(window as any).base?.pay) {
    throw new Error('Base Account SDK not available');
  }

  const testnet = BASE_CHAIN_ID !== BASE_MAINNET_CHAIN_ID;
  return (window as any).base.pay({ amount, to, testnet });
}

export async function getBasePaymentStatus(id: string): Promise<PaymentStatus> {
  if (typeof window === 'undefined' || !(window as any).base?.getPaymentStatus) {
    throw new Error('Base Account SDK not available');
  }

  const testnet = BASE_CHAIN_ID !== BASE_MAINNET_CHAIN_ID;
  return (window as any).base.getPaymentStatus({ id, testnet });
}

export function isBasePayAvailable(): boolean {
  return typeof window !== 'undefined' && !!(window as any).base?.pay;
}
