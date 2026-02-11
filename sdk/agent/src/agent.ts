import { Address, Hex, PublicClient, WalletClient, parseEventLogs } from 'viem';

import { PositionTracker, RiskManager } from './risk';
import { Strategy } from './strategy';
import {
  AgentMetrics,
  AgentStatus,
  MarketData,
  OrderParams,
  Outcome,
  TradeResult,
  TradingAgentConfig,
} from './types';

const MARKET_CORE_ABI = [
  {
    type: 'function',
    name: 'createMarket',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'questionHash', type: 'bytes32' },
      { name: 'closeTime', type: 'uint64' },
      { name: 'resolver', type: 'address' },
    ],
    outputs: [{ name: 'marketId', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'markets',
    stateMutability: 'view',
    inputs: [{ name: 'marketId', type: 'uint256' }],
    outputs: [
      { name: 'questionHash', type: 'bytes32' },
      { name: 'closeTime', type: 'uint64' },
      { name: 'resolveTime', type: 'uint64' },
      { name: 'resolver', type: 'address' },
      { name: 'resolved', type: 'bool' },
      { name: 'outcome', type: 'bool' },
    ],
  },
] as const;

const ORDER_BOOK_ABI = [
  {
    type: 'function',
    name: 'placeOrder',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'marketId', type: 'uint256' },
      { name: 'isYes', type: 'bool' },
      { name: 'priceBps', type: 'uint128' },
      { name: 'size', type: 'uint128' },
      { name: 'expiry', type: 'uint64' },
    ],
    outputs: [{ name: 'orderId', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'cancelOrder',
    stateMutability: 'nonpayable',
    inputs: [{ name: 'orderId', type: 'uint256' }],
    outputs: [],
  },
  {
    type: 'function',
    name: 'claim',
    stateMutability: 'nonpayable',
    inputs: [{ name: 'marketId', type: 'uint256' }],
    outputs: [{ name: 'payout', type: 'uint256' }],
  },
] as const;

const MARKET_CREATED_EVENT = [
  {
    type: 'event',
    name: 'MarketCreated',
    inputs: [
      { indexed: true, name: 'marketId', type: 'uint256' },
      { indexed: true, name: 'questionHash', type: 'bytes32' },
      { indexed: false, name: 'closeTime', type: 'uint64' },
      { indexed: false, name: 'resolver', type: 'address' },
    ],
  },
] as const;

const ORDER_PLACED_EVENT = [
  {
    type: 'event',
    name: 'OrderPlaced',
    inputs: [
      { indexed: true, name: 'orderId', type: 'uint256' },
      { indexed: true, name: 'maker', type: 'address' },
      { indexed: true, name: 'marketId', type: 'uint256' },
      { indexed: false, name: 'isYes', type: 'bool' },
      { indexed: false, name: 'priceBps', type: 'uint128' },
      { indexed: false, name: 'size', type: 'uint128' },
      { indexed: false, name: 'expiry', type: 'uint64' },
    ],
  },
] as const;

export interface TradingAgentOptions {
  publicClient: PublicClient;
  walletClient: WalletClient;
  marketCoreAddress: Address;
  orderBookAddress: Address;
  evmWriteApiUrl?: string;
  config: TradingAgentConfig;
}

interface PreparedEvmWriteTx {
  chain_id: number;
  to: Address;
  data: Hex;
  value: Hex;
}

export class TradingAgent {
  private strategy: Strategy | null = null;
  private riskManager: RiskManager;
  private positionTracker = new PositionTracker();
  private status = AgentStatus.Paused;
  private pollHandle: ReturnType<typeof setInterval> | null = null;
  private tradesCount = 0n;
  private writeApiUrl: string;

  constructor(private readonly options: TradingAgentOptions) {
    this.riskManager = new RiskManager(options.config);
    this.writeApiUrl = (options.evmWriteApiUrl || 'http://localhost:8080/v1').replace(/\/$/, '');
  }

  setStrategy(strategy: Strategy): void {
    this.strategy = strategy;
  }

  async createMarket(params: {
    question: string;
    closeTime: bigint;
    resolver: Address;
    description?: string;
    category?: string;
    resolutionSource?: string;
  }): Promise<{ marketId: bigint; txHash: Hex }> {
    const account = this.requireAccount();
    const prepared = await this.callWriteApi<PreparedEvmWriteTx>('/evm/write/markets/create', {
      from: account,
      question: params.question,
      description: params.description || '',
      category: params.category || '',
      resolutionSource: params.resolutionSource || '',
      closeTime: Number(params.closeTime),
      resolver: params.resolver,
    });

    const txHash = await this.options.walletClient.sendTransaction({
      account,
      chain: this.options.walletClient.chain,
      to: prepared.to,
      data: prepared.data,
      value: BigInt(prepared.value),
    });

    const receipt = await this.options.publicClient.waitForTransactionReceipt({ hash: txHash });
    const [event] = parseEventLogs({
      abi: MARKET_CREATED_EVENT,
      eventName: 'MarketCreated',
      logs: receipt.logs,
    });
    if (!event?.args.marketId) {
      throw new Error('MarketCreated event not found');
    }

    return {
      marketId: event.args.marketId,
      txHash,
    };
  }

  async placeOrder(order: OrderParams): Promise<TradeResult> {
    const validation = this.riskManager.validateTrade(order);
    if (!validation.valid) {
      return {
        success: false,
        error: validation.failedChecks.map((item) => item.message).join(', '),
      };
    }

    const account = this.requireAccount();
    try {
      const prepared = await this.callWriteApi<PreparedEvmWriteTx>('/evm/write/orders/place', {
        from: account,
        marketId: Number(order.marketId),
        outcome: order.outcome === Outcome.Yes ? 'yes' : 'no',
        priceBps: order.priceBps,
        size: order.quantity.toString(),
        expiry: Math.floor(Date.now() / 1000) + (order.expirySeconds || 24 * 60 * 60),
      });
      const txHash = await this.options.walletClient.sendTransaction({
        account,
        chain: this.options.walletClient.chain,
        to: prepared.to,
        data: prepared.data,
        value: BigInt(prepared.value),
      });

