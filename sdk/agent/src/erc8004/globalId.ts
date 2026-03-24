import { keccak256, stringToHex } from 'viem';

import { GlobalAgentId } from './types';

export function parseGlobalId(globalId: string): GlobalAgentId {
  const parts = globalId.split(':');
  if (parts.length !== 4 || parts[0] !== 'eip155') {
    throw new Error(`Invalid global ID: ${globalId}`);
  }

  const chainId = Number.parseInt(parts[1], 10);
  if (!Number.isFinite(chainId)) {
    throw new Error(`Invalid chain ID: ${parts[1]}`);
  }
  if (!/^0x[0-9a-fA-F]{40}$/.test(parts[2])) {
    throw new Error(`Invalid registry address: ${parts[2]}`);
  }

  let agentId: bigint;
  try {
    agentId = BigInt(parts[3]);
  } catch {
    throw new Error(`Invalid agent ID: ${parts[3]}`);
  }

  return {
    namespace: 'eip155',
    chainId,
    registry: parts[2].toLowerCase(),
    agentId,
    raw: globalId,
  };
}

export function formatGlobalId(chainId: number, registry: string, agentId: bigint): string {
  if (!/^0x[0-9a-fA-F]{40}$/.test(registry)) {
    throw new Error(`Invalid registry address: ${registry}`);
  }
  return `eip155:${chainId}:${registry.toLowerCase()}:${agentId.toString()}`;
}

export function isValidGlobalId(globalId: string): boolean {
  try {
    parseGlobalId(globalId);
    return true;
  } catch {
    return false;
  }
}

export function hashGlobalId(globalId: string): string {
  return keccak256(stringToHex(globalId));
}
