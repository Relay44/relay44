'use client';

import { useState, useMemo, useCallback, useRef } from 'react';
import { cn } from '@/lib/utils';
import type { CurvePoint, DistributionPosition } from '@/types/distribution';

export interface DistributionChartProps {
  curveData: CurvePoint[];
  outcomeMin: number;
  outcomeMax: number;
  outcomeUnit?: string;
  marketMu?: number;
  marketSigma?: number;
  userPositions?: DistributionPosition[];
  onHover?: (point: { x: number; pdf: number; cdf: number } | null) => void;
  /**
   * When set, the chart becomes draggable. Click + drag horizontally inside
   * the plot area to update the proposal mean. The chart fires this callback
   * with the new mean value (clamped to [outcomeMin, outcomeMax]).
   */
  onProposalMuChange?: (mu: number) => void;
  /**
   * Current proposal mean. When provided alongside onProposalMuChange, the
   * chart renders an interactive marker at this position.
   */
  proposalMu?: number;
  className?: string;
}

const CHART_W = 800;
const CHART_H = 400;
const PAD_L = 60;
const PAD_R = 20;
const PAD_T = 30;
const PAD_B = 50;
const PLOT_W = CHART_W - PAD_L - PAD_R;
const PLOT_H = CHART_H - PAD_T - PAD_B;

function buildSmoothPath(points: { x: number; y: number }[]): string {
  if (points.length < 2) return '';
  const parts: string[] = [`M ${points[0].x} ${points[0].y}`];
  for (let i = 1; i < points.length; i++) {
    const prev = points[i - 1];
    const curr = points[i];
    const cpx1 = prev.x + (curr.x - prev.x) / 3;
    const cpx2 = curr.x - (curr.x - prev.x) / 3;
    parts.push(`C ${cpx1} ${prev.y}, ${cpx2} ${curr.y}, ${curr.x} ${curr.y}`);
  }
  return parts.join(' ');
}

function generateTicks(min: number, max: number, count: number): number[] {
  const range = max - min;
  const step = range / (count - 1);
  return Array.from({ length: count }, (_, i) => min + step * i);
}

function formatTickValue(value: number): string {
  if (Math.abs(value) >= 1000) {
    return `${(value / 1000).toFixed(1)}k`;
  }
  if (Math.abs(value) >= 1) {
    return value.toFixed(1);
  }
  return value.toFixed(3);
}

export function DistributionChart({
  curveData,
  outcomeMin,
  outcomeMax,
  outcomeUnit,
  marketMu,
  marketSigma,
  userPositions,
  onHover,
  onProposalMuChange,
  proposalMu,
  className,
}: DistributionChartProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [hoverX, setHoverX] = useState<number | null>(null);
  const [isDragging, setIsDragging] = useState(false);

  const interactive = typeof onProposalMuChange === 'function';

  const muFromClientX = useCallback(
    (clientX: number): number | null => {
      const svg = svgRef.current;
      if (!svg) return null;
      const rect = svg.getBoundingClientRect();
      const svgX = ((clientX - rect.left) / rect.width) * CHART_W;
      const clampedSvgX = Math.max(PAD_L, Math.min(PAD_L + PLOT_W, svgX));
      const ratio = (clampedSvgX - PAD_L) / PLOT_W;
      return outcomeMin + ratio * (outcomeMax - outcomeMin);
    },
    [outcomeMin, outcomeMax],
  );

  const maxPdf = useMemo(() => {
    let m = 0;
    for (const pt of curveData) {
      if (pt.marketPdf > m) m = pt.marketPdf;
      if (pt.proposalPdf !== undefined && pt.proposalPdf > m) m = pt.proposalPdf;
    }
    return m * 1.1 || 1;
  }, [curveData]);

  const hasProposal = useMemo(
    () => curveData.some((pt) => pt.proposalPdf !== undefined),
    [curveData]
  );

  const scaleX = useCallback(
    (val: number) => PAD_L + ((val - outcomeMin) / (outcomeMax - outcomeMin)) * PLOT_W,
    [outcomeMin, outcomeMax]
  );

  const scaleY = useCallback(
    (val: number) => PAD_T + PLOT_H - (val / maxPdf) * PLOT_H,
    [maxPdf]
  );

  const { marketPath, marketFillPath, proposalPath, proposalFillPath } = useMemo(() => {
    const mPoints = curveData.map((pt) => ({ x: scaleX(pt.x), y: scaleY(pt.marketPdf) }));
    const mp = buildSmoothPath(mPoints);
    const mfp =
      mPoints.length > 1
        ? `${mp} L ${mPoints[mPoints.length - 1].x} ${PAD_T + PLOT_H} L ${mPoints[0].x} ${PAD_T + PLOT_H} Z`
        : '';

    let pp = '';
    let pfp = '';
    if (hasProposal) {
      const pPoints = curveData
        .filter((pt) => pt.proposalPdf !== undefined)
        .map((pt) => ({ x: scaleX(pt.x), y: scaleY(pt.proposalPdf!) }));
      pp = buildSmoothPath(pPoints);
      if (pPoints.length > 1) {
        pfp = `${pp} L ${pPoints[pPoints.length - 1].x} ${PAD_T + PLOT_H} L ${pPoints[0].x} ${PAD_T + PLOT_H} Z`;
      }
    }

    return { marketPath: mp, marketFillPath: mfp, proposalPath: pp, proposalFillPath: pfp };
  }, [curveData, scaleX, scaleY, hasProposal]);

  const xTicks = useMemo(() => generateTicks(outcomeMin, outcomeMax, 7), [outcomeMin, outcomeMax]);

  const hoverData = useMemo(() => {
    if (hoverX === null || curveData.length === 0) return null;
    const xVal = outcomeMin + ((hoverX - PAD_L) / PLOT_W) * (outcomeMax - outcomeMin);
    if (xVal < outcomeMin || xVal > outcomeMax) return null;

    let closest = curveData[0];
    let minDist = Math.abs(curveData[0].x - xVal);
    for (let i = 1; i < curveData.length; i++) {
      const dist = Math.abs(curveData[i].x - xVal);
      if (dist < minDist) {
        minDist = dist;
        closest = curveData[i];
      }
    }
    return closest;
  }, [hoverX, curveData, outcomeMin, outcomeMax]);

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<SVGSVGElement>) => {
      if (!svgRef.current) return;
      const rect = svgRef.current.getBoundingClientRect();
      const svgX = ((e.clientX - rect.left) / rect.width) * CHART_W;

      if (isDragging && interactive) {
        const newMu = muFromClientX(e.clientX);
        if (newMu !== null) onProposalMuChange?.(newMu);
      }

      if (svgX >= PAD_L && svgX <= PAD_L + PLOT_W) {
        setHoverX(svgX);
        if (onHover) {
          const xVal = outcomeMin + ((svgX - PAD_L) / PLOT_W) * (outcomeMax - outcomeMin);
          let closest = curveData[0];
          let minDist = Math.abs(curveData[0].x - xVal);
          for (let i = 1; i < curveData.length; i++) {
            const dist = Math.abs(curveData[i].x - xVal);
            if (dist < minDist) {
              minDist = dist;
              closest = curveData[i];
            }
          }
          if (closest) {
            onHover({ x: closest.x, pdf: closest.marketPdf, cdf: closest.cdf });
          }
        }
      } else {
        setHoverX(null);
        onHover?.(null);
      }
    },
    [
      curveData,
      outcomeMin,
      outcomeMax,
      onHover,
      isDragging,
      interactive,
      muFromClientX,
      onProposalMuChange,
    ]
  );

  const handleMouseLeave = useCallback(() => {
    setHoverX(null);
    setIsDragging(false);
    onHover?.(null);
  }, [onHover]);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent<SVGSVGElement>) => {
      if (!interactive) return;
      const newMu = muFromClientX(e.clientX);
      if (newMu === null) return;
      setIsDragging(true);
      onProposalMuChange?.(newMu);
    },
    [interactive, muFromClientX, onProposalMuChange],
  );

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  if (curveData.length === 0) {
    return (
      <div className={cn('flex items-center justify-center h-64 border border-border', className)}>
        <span className="text-text-secondary text-xs">No curve data available</span>
      </div>
    );
  }

  return (
    <div className={cn('relative', className)}>
      <svg
        ref={svgRef}
        viewBox={`0 0 ${CHART_W} ${CHART_H}`}
        className={cn(
          'w-full h-auto select-none',
          interactive && (isDragging ? 'cursor-grabbing' : 'cursor-grab'),
        )}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
      >
        <defs>
          <linearGradient id="dist-market-fill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--color-text-primary)" stopOpacity="0.12" />
            <stop offset="100%" stopColor="var(--color-text-primary)" stopOpacity="0.02" />
          </linearGradient>
          <linearGradient id="dist-proposal-fill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--color-bid)" stopOpacity="0.2" />
            <stop offset="100%" stopColor="var(--color-bid)" stopOpacity="0.02" />
          </linearGradient>
        </defs>

        {/* Grid lines */}
        {xTicks.map((tick) => (
          <line
            key={tick}
            x1={scaleX(tick)}
            y1={PAD_T}
            x2={scaleX(tick)}
            y2={PAD_T + PLOT_H}
            stroke="var(--color-border)"
            strokeWidth="0.5"
            strokeDasharray="4 4"
          />
        ))}

        {/* Market curve fill */}
        {marketFillPath && (
          <path d={marketFillPath} fill="url(#dist-market-fill)" />
        )}

        {/* Proposal curve fill */}
        {hasProposal && proposalFillPath && (
          <path d={proposalFillPath} fill="url(#dist-proposal-fill)" />
        )}

        {/* Market curve stroke */}
        <path
          d={marketPath}
          fill="none"
          stroke="var(--color-text-primary)"
          strokeWidth="2"
        />

        {/* Proposal curve stroke */}
        {hasProposal && proposalPath && (
          <path
            d={proposalPath}
            fill="none"
            stroke="var(--color-bid)"
            strokeWidth="2"
            strokeDasharray="6 3"
          />
        )}

        {/* Mean indicator */}
        {marketMu !== undefined && (
          <>
            <line
              x1={scaleX(marketMu)}
              y1={PAD_T}
              x2={scaleX(marketMu)}
              y2={PAD_T + PLOT_H}
              stroke="var(--color-accent)"
              strokeWidth="1"
              strokeDasharray="4 2"
              opacity="0.6"
            />
            <text
              x={scaleX(marketMu)}
              y={PAD_T - 6}
              textAnchor="middle"
              fill="var(--color-accent)"
              fontSize="9"
              fontFamily="monospace"
              opacity="0.8"
            >
              Crowd {formatTickValue(marketMu)}
            </text>
          </>
        )}

        {/* Interactive proposal mean indicator (drag-the-curve) */}
        {interactive && proposalMu !== undefined &&
          proposalMu >= outcomeMin && proposalMu <= outcomeMax && (
            <>
              <line
                x1={scaleX(proposalMu)}
                y1={PAD_T}
                x2={scaleX(proposalMu)}
                y2={PAD_T + PLOT_H}
                stroke="var(--color-bid)"
                strokeWidth="1.5"
                strokeDasharray="4 2"
                opacity="0.9"
              />
              <circle
                cx={scaleX(proposalMu)}
                cy={PAD_T + PLOT_H / 2}
                r={isDragging ? 7 : 5}
                fill="var(--color-bid)"
                stroke="var(--color-bg-primary)"
                strokeWidth="2"
                opacity="0.95"
              />
              <text
                x={scaleX(proposalMu)}
                y={PAD_T + PLOT_H + 16}
                textAnchor="middle"
                fill="var(--color-bid)"
                fontSize="10"
                fontFamily="monospace"
                fontWeight="600"
              >
                You {formatTickValue(proposalMu)}
              </text>
            </>
          )}

        {/* User position markers */}
        {userPositions?.map((pos) => {
          const px = scaleX(pos.mu);
          const inBounds = pos.mu >= outcomeMin && pos.mu <= outcomeMax;
          if (!inBounds) return null;
          return (
            <g key={pos.id}>
              <line
                x1={px}
                y1={PAD_T + 8}
                x2={px}
                y2={PAD_T + PLOT_H}
                stroke="var(--color-accent)"
                strokeWidth="1.5"
                opacity="0.7"
              />
              <polygon
                points={`${px},${PAD_T + 4} ${px - 5},${PAD_T + 12} ${px + 5},${PAD_T + 12}`}
                fill="var(--color-accent)"
                opacity="0.9"
              />
              <title>{`mu: ${pos.mu.toFixed(2)}, sigma: ${pos.sigma.toFixed(2)}, size: ${pos.size}`}</title>
            </g>
          );
        })}

        {/* X-axis ticks */}
        {xTicks.map((tick) => (
          <g key={`tick-${tick}`}>
            <line
              x1={scaleX(tick)}
              y1={PAD_T + PLOT_H}
              x2={scaleX(tick)}
              y2={PAD_T + PLOT_H + 6}
              stroke="var(--color-text-secondary)"
              strokeWidth="1"
            />
            <text
              x={scaleX(tick)}
              y={PAD_T + PLOT_H + 20}
              textAnchor="middle"
              fill="var(--color-text-secondary)"
              fontSize="11"
              fontFamily="monospace"
            >
              {formatTickValue(tick)}
            </text>
          </g>
        ))}

        {/* X-axis unit label */}
        {outcomeUnit && (
          <text
            x={PAD_L + PLOT_W / 2}
            y={CHART_H - 4}
            textAnchor="middle"
            fill="var(--color-text-muted)"
            fontSize="10"
          >
            {outcomeUnit}
          </text>
        )}

        {/* Hover crosshair */}
        {hoverX !== null && hoverData && (
          <>
            <line
              x1={hoverX}
              y1={PAD_T}
              x2={hoverX}
              y2={PAD_T + PLOT_H}
              stroke="var(--color-border)"
              strokeWidth="1"
            />
            <circle
              cx={scaleX(hoverData.x)}
              cy={scaleY(hoverData.marketPdf)}
              r="4"
              fill="var(--color-text-primary)"
              stroke="var(--color-bg-primary)"
              strokeWidth="2"
            />
            {hoverData.proposalPdf !== undefined && (
              <circle
                cx={scaleX(hoverData.x)}
                cy={scaleY(hoverData.proposalPdf)}
                r="4"
                fill="var(--color-bid)"
                stroke="var(--color-bg-primary)"
                strokeWidth="2"
              />
            )}
          </>
        )}

        {/* Legend */}
        <g transform={`translate(${CHART_W - 180}, ${PAD_T + 8})`}>
          <line x1="0" y1="0" x2="20" y2="0" stroke="var(--color-text-primary)" strokeWidth="2" />
          <text x="26" y="4" fill="var(--color-text-secondary)" fontSize="10">
            Current Market
          </text>
          {hasProposal && (
            <>
              <line
                x1="0"
                y1="16"
                x2="20"
                y2="16"
                stroke="var(--color-bid)"
                strokeWidth="2"
                strokeDasharray="6 3"
              />
              <text x="26" y="20" fill="var(--color-text-secondary)" fontSize="10">
                Your Proposal
              </text>
            </>
          )}
          {userPositions && userPositions.length > 0 && (
            <>
              <line
                x1="0"
                y1={hasProposal ? 32 : 16}
                x2="20"
                y2={hasProposal ? 32 : 16}
                stroke="var(--color-accent)"
                strokeWidth="1.5"
              />
              <polygon
                points={`10,${hasProposal ? 27 : 11} 7,${hasProposal ? 33 : 17} 13,${hasProposal ? 33 : 17}`}
                fill="var(--color-accent)"
              />
              <text
                x="26"
                y={hasProposal ? 36 : 20}
                fill="var(--color-text-secondary)"
                fontSize="10"
              >
                Your Positions
              </text>
            </>
          )}
        </g>
      </svg>

      {/* Hover tooltip */}
      {hoverX !== null && hoverData && (
        <div
          className="absolute pointer-events-none bg-bg-secondary border border-border px-3 py-2 text-xs"
          style={{
            left: `${(hoverX / CHART_W) * 100}%`,
            top: '8px',
            transform: hoverX > CHART_W / 2 ? 'translateX(-110%)' : 'translateX(10%)',
          }}
        >
          <div className="font-mono tabular-nums text-text-primary">
            Value{outcomeUnit ? ` (${outcomeUnit})` : ''}: {hoverData.x.toFixed(2)}
          </div>
          <div className="font-mono tabular-nums text-text-secondary">
            Probability: {(hoverData.marketPdf * 100).toFixed(1)}%
          </div>
          <div className="font-mono tabular-nums text-text-secondary">
            Cumulative: {(hoverData.cdf * 100).toFixed(1)}%
          </div>
        </div>
      )}
    </div>
  );
}
