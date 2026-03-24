import { MarketData, OrderParams, OrderType, Outcome, Signal } from './types';

export interface Strategy {
  name: string;
  analyze(data: MarketData): Signal | null;
  toOrder(signal: Signal, bankroll: bigint): OrderParams | null;
}

export class MomentumStrategy implements Strategy {
  name = 'momentum';

  constructor(
    private readonly minEdge = 0.05,
    private readonly maxConfidence = 1
  ) {}

  analyze(data: MarketData): Signal | null {
    if (data.resolved) return null;

    const deviation = (data.yesPriceBps - 5000) / 5000;
    if (Math.abs(deviation) < this.minEdge) return null;

    const bullish = deviation > 0;
    const confidence = Math.min(Math.abs(deviation) / this.minEdge, this.maxConfidence);
    return {
      marketId: data.marketId,
      direction: bullish ? 'buy_yes' : 'buy_no',
      confidence,
      targetPriceBps: Math.max(100, Math.min(9900, data.yesPriceBps + (bullish ? 50 : -50))),
      reason: bullish ? 'Momentum bullish' : 'Momentum bearish',
    };
  }

  toOrder(signal: Signal, bankroll: bigint): OrderParams | null {
    if (signal.confidence < 0.4) return null;

    const stakeBps = BigInt(Math.floor(signal.confidence * 1000));
    const quantity = (bankroll * stakeBps) / 10000n;
    if (quantity <= 0n) return null;

    return {
      marketId: signal.marketId,
      outcome: signal.direction === 'buy_yes' ? Outcome.Yes : Outcome.No,
      priceBps: signal.targetPriceBps,
      quantity,
      orderType: OrderType.Limit,
    };
  }
}

export class MeanReversionStrategy implements Strategy {
  name = 'mean_reversion';

  constructor(private readonly thresholdBps = 1200) {}

  analyze(data: MarketData): Signal | null {
    if (data.resolved) return null;

    const delta = data.yesPriceBps - 5000;
    if (Math.abs(delta) < this.thresholdBps) return null;

    const buyYes = delta < 0;
    return {
      marketId: data.marketId,
      direction: buyYes ? 'buy_yes' : 'buy_no',
      confidence: Math.min(Math.abs(delta) / this.thresholdBps, 1),
      targetPriceBps: 5000,
      reason: buyYes ? 'Yes reverted below mean' : 'No reverted below mean',
    };
  }

  toOrder(signal: Signal, bankroll: bigint): OrderParams | null {
    if (signal.confidence < 0.3) return null;
    const quantity = (bankroll * BigInt(Math.floor(signal.confidence * 800))) / 10000n;
    if (quantity <= 0n) return null;

    return {
      marketId: signal.marketId,
      outcome: signal.direction === 'buy_yes' ? Outcome.Yes : Outcome.No,
      priceBps: signal.targetPriceBps,
      quantity,
      orderType: OrderType.Limit,
    };
  }
}

export class CompositeStrategy implements Strategy {
  name = 'composite';

  constructor(private readonly strategies: Array<{ strategy: Strategy; weight: number }>) {}

  analyze(data: MarketData): Signal | null {
    const weightedSignals: Array<{ signal: Signal; weight: number }> = [];
    for (const { strategy, weight } of this.strategies) {
      const signal = strategy.analyze(data);
      if (signal) weightedSignals.push({ signal, weight });
    }
    if (weightedSignals.length === 0) return null;

    let yesScore = 0;
    let noScore = 0;
    for (const { signal, weight } of weightedSignals) {
      if (signal.direction === 'buy_yes') yesScore += signal.confidence * weight;
      else noScore += signal.confidence * weight;
    }

    const totalWeight = this.strategies.reduce((sum, entry) => sum + entry.weight, 0) || 1;
    const normalizedYes = yesScore / totalWeight;
    const normalizedNo = noScore / totalWeight;
    if (Math.max(normalizedYes, normalizedNo) < 0.3) return null;

    const direction = normalizedYes >= normalizedNo ? 'buy_yes' : 'buy_no';
    const averageTarget = Math.round(
      weightedSignals.reduce((sum, item) => sum + item.signal.targetPriceBps * item.weight, 0) /
      weightedSignals.reduce((sum, item) => sum + item.weight, 0)
    );

    return {
      marketId: data.marketId,
      direction,
      confidence: Math.max(normalizedYes, normalizedNo),
      targetPriceBps: averageTarget,
      reason: `${weightedSignals.length} strategy consensus`,
    };
  }

  toOrder(signal: Signal, bankroll: bigint): OrderParams | null {
    const quantity = (bankroll * BigInt(Math.floor(signal.confidence * 1000))) / 10000n;
    if (quantity <= 0n) return null;

    return {
      marketId: signal.marketId,
      outcome: signal.direction === 'buy_yes' ? Outcome.Yes : Outcome.No,
      priceBps: signal.targetPriceBps,
      quantity,
      orderType: OrderType.Limit,
    };
  }
}
