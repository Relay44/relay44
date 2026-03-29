'use client';

import {
  api,
  type ExternalOrderIntent,
  type ExternalOrderRecord,
  type PreparedExternalProviderRequest,
} from '@/lib/api';
import type { Outcome, OrderSide } from '@/types';

type ExternalProvider = 'limitless' | 'polymarket';
type SignedOrder = Record<string, unknown>;
type EthereumProvider = {
  request: (args: Record<string, unknown>) => Promise<unknown>;
};

function readTypedData(intent: ExternalOrderIntent): Record<string, unknown> {
  const rawIntent = intent as unknown as Record<string, unknown>;
  const typedData = (rawIntent.typedData ?? rawIntent.typed_data) as
    | Record<string, unknown>
    | undefined;
  if (!typedData) {
    throw new Error('External order intent did not include typed data');
  }
  return typedData;
}

function parseSignedOrder(input?: string): SignedOrder | null {
  if (!input?.trim()) {
    return null;
  }
  return JSON.parse(input) as SignedOrder;
}

function browserEthereum(): EthereumProvider {
  const ethereum = (window as unknown as { ethereum?: EthereumProvider }).ethereum;
  if (!ethereum) {
    throw new Error('Connect wallet to sign typed data');
  }
  return ethereum;
}

async function signTypedData(
  walletAddress: string,
  typedData: Record<string, unknown>
): Promise<SignedOrder> {
  if (!walletAddress) {
    throw new Error('Connect wallet to sign typed data');
  }

  const signature = await browserEthereum().request({
    method: 'eth_signTypedData_v4',
    params: [walletAddress, JSON.stringify(typedData)],
  });

  return {
    typedData,
    signature: String(signature || ''),
  };
}

function parseProviderPayload(text: string): Record<string, unknown> {
  if (!text.trim()) {
    return {};
  }

  try {
    const parsed = JSON.parse(text) as unknown;
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
    return { value: parsed };
  } catch {
    return { error: text };
  }
}

async function executePreparedRequest(
  request: PreparedExternalProviderRequest
): Promise<{ status: number; payload: Record<string, unknown> }> {
  const response = await fetch(request.url, {
    method: request.method,
    headers: request.headers,
    body: request.body,
  });
  const text = await response.text();

  return {
    status: response.status,
    payload: parseProviderPayload(text),
  };
}

export async function submitExternalMarketOrder(input: {
  provider: ExternalProvider;
  marketId: string;
  outcome: Outcome;
  side: OrderSide;
  price: number;
  quantity: number;
  credentialId?: string;
  walletAddress: string;
  signedOrderJson?: string;
}): Promise<{ intent: ExternalOrderIntent; order: ExternalOrderRecord; signedOrder: SignedOrder }> {
  const intent = await api.createExternalOrderIntent({
    provider: input.provider,
    marketId: input.marketId,
    outcome: input.outcome,
    side: input.side,
    price: input.price,
    quantity: input.quantity,
    credentialId: input.credentialId,
  });
  const typedData = readTypedData(intent);
  const signedOrder =
    parseSignedOrder(input.signedOrderJson) ??
    (await signTypedData(input.walletAddress, typedData));

  if (input.provider === 'polymarket') {
    const prepared = await api.prepareExternalOrderSubmit({
      intentId: intent.id,
      signedOrder,
      credentialId: input.credentialId,
    });
    const providerResult = await executePreparedRequest(prepared);
    const order = await api.submitExternalOrder({
      intentId: intent.id,
      signedOrder,
      credentialId: input.credentialId,
      providerResponse: providerResult.payload,
      providerStatus: providerResult.status,
    });

    return { intent, order, signedOrder };
  }

  const order = await api.submitExternalOrder({
    intentId: intent.id,
    signedOrder,
    credentialId: input.credentialId,
  });

  return { intent, order, signedOrder };
}

export async function cancelExternalMarketOrder(input: {
  provider: ExternalProvider;
  providerOrderId: string;
  credentialId?: string;
  payload?: Record<string, unknown>;
}): Promise<{ ok: boolean }> {
  if (input.provider === 'polymarket') {
    const prepared = await api.prepareExternalOrderCancel({
      provider: input.provider,
      providerOrderId: input.providerOrderId,
      credentialId: input.credentialId,
      payload: input.payload,
    });
    const providerResult = await executePreparedRequest(prepared);

    return api.cancelExternalOrder({
      provider: input.provider,
      providerOrderId: input.providerOrderId,
      credentialId: input.credentialId,
      payload: input.payload,
      providerResponse: providerResult.payload,
      providerStatus: providerResult.status,
    });
  }

  return api.cancelExternalOrder(input);
}
