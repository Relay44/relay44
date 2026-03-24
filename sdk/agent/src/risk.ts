import { OrderParams, PositionSizing, RiskParams, TradingAgentConfig } from './types';

export interface ValidationCheck {
  name: string;
  passed: boolean;
  message: string;
  value?: number;
  limit?: number;
}

export interface ValidationResult {
  valid: boolean;
  checks: ValidationCheck[];
  failedChecks: ValidationCheck[];
}

export class RiskManager {
  constructor(private readonly agent: TradingAgentConfig) {}

  validateTrade(order: OrderParams): ValidationResult {
    const checks = [
      this.checkPositionSize(order),
      this.checkExposure(order),
      this.checkDrawdown(),
      this.checkDailyLoss(),
    ];
    const failedChecks = checks.filter((check) => !check.passed);
    return {
      valid: failedChecks.length === 0,
      checks,
      failedChecks,
    };
  }

  calculatePositionSize(edgeBps: number): bigint {
    const bankroll = this.agent.availableBalance;
    const params = this.agent.riskParams;

    switch (params.positionSizing) {
      case PositionSizing.Fixed:
        return params.sizingParam;
      case PositionSizing.Kelly: {
        const fraction = Math.max(0, edgeBps) / 10_000;
        return BigInt(Math.floor(Number(bankroll) * fraction * Number(params.sizingParam) / 10_000));
      }
      case PositionSizing.Proportional:
        return (bankroll * params.sizingParam) / 10_000n;
      default:
        return params.sizingParam;
    }
  }

  private checkPositionSize(order: OrderParams): ValidationCheck {
    const passed = order.quantity <= this.agent.maxPositionSize;
    return {
      name: 'position_size',
      passed,
      message: passed
        ? 'Position size within configured max'
        : 'Position size exceeds configured max',
      value: Number(order.quantity),
      limit: Number(this.agent.maxPositionSize),
    };
  }

  private checkExposure(order: OrderParams): ValidationCheck {
    const projected = this.agent.lockedBalance + order.quantity;
    const passed = projected <= this.agent.maxTotalExposure;
    return {
      name: 'total_exposure',
      passed,
      message: passed
        ? 'Projected exposure within limit'
        : 'Projected exposure exceeds limit',
      value: Number(projected),
      limit: Number(this.agent.maxTotalExposure),
    };
  }

  private checkDrawdown(): ValidationCheck {
    if (this.agent.highWaterMark === 0n || this.agent.riskParams.maxDrawdownBps <= 0) {
      return { name: 'drawdown', passed: true, message: 'Drawdown check disabled' };
    }

    const drawdown = Number((this.agent.currentDrawdown * 10_000n) / this.agent.highWaterMark);
    const passed = drawdown <= this.agent.riskParams.maxDrawdownBps;
    return {
      name: 'drawdown',
      passed,
      message: passed ? 'Drawdown within limit' : 'Drawdown exceeds limit',
      value: drawdown,
      limit: this.agent.riskParams.maxDrawdownBps,
    };
  }

  private checkDailyLoss(): ValidationCheck {
    const limit = this.agent.riskParams.maxDailyLoss;
    if (limit === 0n) {
      return { name: 'daily_loss', passed: true, message: 'Daily loss check disabled' };
    }

    const passed = this.agent.dailyLoss < limit;
    return {
      name: 'daily_loss',
      passed,
      message: passed ? 'Daily loss within limit' : 'Daily loss exceeds limit',
      value: Number(this.agent.dailyLoss),
      limit: Number(limit),
    };
  }
}

export interface Position {
  marketId: bigint;
  outcome: number;
  quantity: bigint;
  avgEntryPriceBps: number;
  openedAt: number;
}

export class PositionTracker {
  private positions = new Map<string, Position>();

  addPosition(next: Position): void {
    const key = `${next.marketId}_${next.outcome}`;
    const current = this.positions.get(key);
    if (!current) {
      this.positions.set(key, next);
      return;
    }

    const total = current.quantity + next.quantity;
    const weightedPrice = (
      Number(current.quantity) * current.avgEntryPriceBps +
      Number(next.quantity) * next.avgEntryPriceBps
    ) / Number(total);

    this.positions.set(key, {
      ...current,
      quantity: total,
      avgEntryPriceBps: Math.round(weightedPrice),
    });
  }

  getAllPositions(): Position[] {
    return Array.from(this.positions.values());
  }
}

export function createDefaultRiskParams(): RiskParams {
  return {
    maxDrawdownBps: 2000,
    maxDailyLoss: 0n,
    minEdgeBps: 50,
    positionSizing: PositionSizing.Proportional,
    sizingParam: 500n,
  };
}
