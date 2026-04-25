import {
  ERC20_ABI,
  MARKET_CORE_ABI,
  MARKET_CREATED_EVENT_ABI,
  ORDER_BOOK_ABI,
  ORDER_PLACED_EVENT_ABI,
  RELAY_STAKING_ABI,
  type ContractAbi,
} from '@relay44/protocol';

export {
  ERC20_ABI,
  MARKET_CORE_ABI,
  MARKET_CREATED_EVENT_ABI,
  ORDER_BOOK_ABI,
  ORDER_PLACED_EVENT_ABI,
  RELAY_STAKING_ABI,
};

export type ContractAbiName =
  | 'market-core'
  | 'order-book'
  | 'erc20'
  | 'relay-staking';

export const DEFAULT_CONTRACTS_BASE_URL = 'https://relay44.com';

interface AbiResponse {
  name: string;
  abi: ContractAbi;
}

export async function fetchContractAbi(
  name: ContractAbiName,
  options: { baseUrl?: string; fetchImpl?: typeof fetch } = {},
): Promise<AbiResponse> {
  const baseUrl = (
    options.baseUrl ||
    (typeof process !== 'undefined' && process.env && process.env.RELAY44_CONTRACTS_URL) ||
    DEFAULT_CONTRACTS_BASE_URL
  ).replace(/\/+$/, '');

  const fetchImpl =
    options.fetchImpl ||
    (typeof fetch === 'function' ? fetch : undefined);
  if (!fetchImpl) {
    throw new Error(
      'fetchContractAbi: global fetch is not available. Pass `fetchImpl` explicitly.',
    );
  }

  const url = `${baseUrl}/api/contracts/${name}/abi`;
  const response = await fetchImpl(url, {
    headers: { accept: 'application/json' },
  });

  if (!response.ok) {
    throw new Error(
      `fetchContractAbi(${name}) failed: ${response.status} ${response.statusText} (${url})`,
    );
  }

  const payload = (await response.json()) as AbiResponse;
  if (!payload || !Array.isArray(payload.abi)) {
    throw new Error(
      `fetchContractAbi(${name}) returned an unexpected payload shape`,
    );
  }

  return payload;
}
