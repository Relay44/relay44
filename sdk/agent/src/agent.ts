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

      const receipt = await this.options.publicClient.waitForTransactionReceipt({ hash: txHash });
      const [event] = parseEventLogs({
        abi: ORDER_PLACED_EVENT,
        eventName: 'OrderPlaced',
        logs: receipt.logs,
      });

      this.tradesCount += 1n;
      this.positionTracker.addPosition({
        marketId: order.marketId,
        outcome: order.outcome,
        quantity: order.quantity,
        avgEntryPriceBps: order.priceBps,
        openedAt: Date.now(),
      });

      return {
        success: true,
        txHash,
        orderId: event?.args.orderId,
      };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }

  async cancelOrder(orderId: bigint): Promise<Hex> {
    const account = this.requireAccount();
    const prepared = await this.callWriteApi<PreparedEvmWriteTx>('/evm/write/orders/cancel', {
      from: account,
      orderId: Number(orderId),
    });
    const txHash = await this.options.walletClient.sendTransaction({
      account,
      chain: this.options.walletClient.chain,
      to: prepared.to,
      data: prepared.data,
      value: BigInt(prepared.value),
    });
    await this.options.publicClient.waitForTransactionReceipt({ hash: txHash });
    return txHash;
  }

  async claim(marketId: bigint): Promise<Hex> {
    const account = this.requireAccount();
    const prepared = await this.callWriteApi<PreparedEvmWriteTx>('/evm/write/positions/claim', {
      from: account,
      marketId: Number(marketId),
    });
    const txHash = await this.options.walletClient.sendTransaction({
      account,
      chain: this.options.walletClient.chain,
      to: prepared.to,
      data: prepared.data,
      value: BigInt(prepared.value),
    });
    await this.options.publicClient.waitForTransactionReceipt({ hash: txHash });
    return txHash;
  }

  async claimFor(owner: Address, marketId: bigint): Promise<Hex> {
    const account = this.requireAccount();
    const prepared = await this.callWriteApi<PreparedEvmWriteTx>('/evm/write/positions/claim-for', {
      from: account,
      user: owner,
      marketId: Number(marketId),
    });
    const txHash = await this.options.walletClient.sendTransaction({
      account,
      chain: this.options.walletClient.chain,
      to: prepared.to,
      data: prepared.data,
      value: BigInt(prepared.value),
    });
    await this.options.publicClient.waitForTransactionReceipt({ hash: txHash });
    return txHash;
  }

  async fetchMarketData(marketId: bigint): Promise<MarketData> {
    const [, , , , resolved, outcome] = await this.options.publicClient.readContract({
      address: this.options.marketCoreAddress,
      abi: MARKET_CORE_ABI,
      functionName: 'markets',
      args: [marketId],
    });

    return {
      marketId,
      yesPriceBps: 5000,
      noPriceBps: 5000,
      matchedVolume: 0n,
      lastUpdate: Date.now(),
      resolved,
      outcome: resolved ? (outcome ? Outcome.Yes : Outcome.No) : undefined,
    };
  }

  async start(markets: bigint[], pollIntervalMs = 5_000): Promise<void> {
    if (!this.strategy) {
      throw new Error('No strategy configured');
    }
    if (this.status === AgentStatus.Active) return;

    this.status = AgentStatus.Active;
    this.pollHandle = setInterval(async () => {
      if (this.status !== AgentStatus.Active || !this.strategy) return;

      for (const marketId of markets) {
        try {
          const marketData = await this.fetchMarketData(marketId);
          const signal = this.strategy.analyze(marketData);
          if (!signal) continue;

          const order = this.strategy.toOrder(signal, this.options.config.availableBalance);
          if (!order) continue;
          await this.placeOrder(order);
        } catch (error) {
          console.error('agent loop error', marketId.toString(), error);
        }
      }
    }, pollIntervalMs);
  }

  stop(): void {
    this.status = AgentStatus.Stopped;
    if (this.pollHandle) {
      clearInterval(this.pollHandle);
      this.pollHandle = null;
    }
  }

  async getMetrics(): Promise<AgentMetrics> {
    const avgPnlPerTrade = this.tradesCount > 0n
      ? this.options.config.totalPnl / this.tradesCount
      : 0n;
    return {
      totalPnl: this.options.config.totalPnl,
      winRate: 0,
      tradesCount: this.tradesCount,
      avgPnlPerTrade,
      maxDrawdownBps: this.options.config.riskParams.maxDrawdownBps,
    };
  }

  private requireAccount(): Address {
    const account = this.options.walletClient.account?.address;
    if (!account) {
      throw new Error('walletClient account is required');
    }
    return account;
  }

  private async callWriteApi<T>(path: string, payload: unknown): Promise<T> {
    const fetchImpl = (globalThis as unknown as { fetch?: (url: string, init: unknown) => Promise<{ ok: boolean; status: number; text: () => Promise<string>; json: () => Promise<T> }> }).fetch;
    if (!fetchImpl) {
      throw new Error('Global fetch is required for SDK write API calls');
    }

    const response = await fetchImpl(`${this.writeApiUrl}${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      const text = await response.text();
      throw new Error(`Write API request failed (${response.status}): ${text}`);
    }
    return response.json();
  }
}

export function createAgent(options: TradingAgentOptions): TradingAgent {
  return new TradingAgent(options);
}
