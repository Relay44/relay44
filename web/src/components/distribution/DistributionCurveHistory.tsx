'use client';

import { useState, useMemo, useCallback, useRef } from 'react';
import { cn } from '@/lib/utils';
import type { CurveSnapshot } from '@/types/distribution';

interface DistributionCurveHistoryProps {
  snapshots: CurveSnapshot[];
  outcomeUnit?: string;
  className?: string;
}

const CHART_W = 800;
const CHART_H = 240;
const PAD_L = 60;
const PAD_R = 20;
const PAD_T = 20;
const PAD_B = 40;
const PLOT_W = CHART_W - PAD_L - PAD_R;
const PLOT_H = CHART_H - PAD_T - PAD_B;

function buildLinePath(points: { x: number; y: number }[]): string {
  if (points.length < 2) return '';
  return points.map((p, i) => (i === 0 ? `M ${p.x} ${p.y}` : `L ${p.x} ${p.y}`)).join(' ');
}

export function DistributionCurveHistory({
  snapshots,
  outcomeUnit,
  className,
}: DistributionCurveHistoryProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [hoverIdx, setHoverIdx] = useState<number | null>(null);

  const { muRange, sigmaRange, timeRange } = useMemo(() => {
    if (snapshots.length === 0) return { muRange: [0, 1], sigmaRange: [0, 1], timeRange: [0, 1] };
    let muMin = Infinity, muMax = -Infinity, sigMin = Infinity, sigMax = -Infinity;
    for (const s of snapshots) {
      if (s.marketMu < muMin) muMin = s.marketMu;
      if (s.marketMu > muMax) muMax = s.marketMu;
      if (s.marketSigma < sigMin) sigMin = s.marketSigma;
      if (s.marketSigma > sigMax) sigMax = s.marketSigma;
    }
    const muPad = (muMax - muMin) * 0.1 || 0.5;
    const sigPad = (sigMax - sigMin) * 0.1 || 0.1;
    const t0 = new Date(snapshots[0].capturedAt).getTime();
    const t1 = new Date(snapshots[snapshots.length - 1].capturedAt).getTime();
    return {
      muRange: [muMin - muPad, muMax + muPad],
      sigmaRange: [sigMin - sigPad, sigMax + sigPad],
      timeRange: [t0, t1 === t0 ? t0 + 1 : t1],
    };
  }, [snapshots]);

  const scaleX = useCallback(
    (t: number) => PAD_L + ((t - timeRange[0]) / (timeRange[1] - timeRange[0])) * PLOT_W,
    [timeRange]
  );

  const scaleMuY = useCallback(
    (v: number) => PAD_T + PLOT_H - ((v - muRange[0]) / (muRange[1] - muRange[0])) * PLOT_H,
    [muRange]
  );

  const scaleSigmaY = useCallback(
    (v: number) => PAD_T + PLOT_H - ((v - sigmaRange[0]) / (sigmaRange[1] - sigmaRange[0])) * PLOT_H,
    [sigmaRange]
  );

  const { muPath, sigmaPath } = useMemo(() => {
    const muPts = snapshots.map((s) => ({
      x: scaleX(new Date(s.capturedAt).getTime()),
      y: scaleMuY(s.marketMu),
    }));
    const sigPts = snapshots.map((s) => ({
      x: scaleX(new Date(s.capturedAt).getTime()),
      y: scaleSigmaY(s.marketSigma),
    }));
    return { muPath: buildLinePath(muPts), sigmaPath: buildLinePath(sigPts) };
  }, [snapshots, scaleX, scaleMuY, scaleSigmaY]);

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<SVGSVGElement>) => {
      if (!svgRef.current || snapshots.length === 0) return;
      const rect = svgRef.current.getBoundingClientRect();
      const svgX = ((e.clientX - rect.left) / rect.width) * CHART_W;
      if (svgX < PAD_L || svgX > PAD_L + PLOT_W) {
        setHoverIdx(null);
        return;
      }
      const t = timeRange[0] + ((svgX - PAD_L) / PLOT_W) * (timeRange[1] - timeRange[0]);
      let closest = 0;
      let minDist = Infinity;
      for (let i = 0; i < snapshots.length; i++) {
        const d = Math.abs(new Date(snapshots[i].capturedAt).getTime() - t);
        if (d < minDist) { minDist = d; closest = i; }
      }
      setHoverIdx(closest);
    },
    [snapshots, timeRange]
  );

  if (snapshots.length < 2) {
    return (
      <div className={cn('flex items-center justify-center h-32 border border-border', className)}>
        <span className="text-text-secondary text-xs">Not enough history data yet</span>
      </div>
    );
  }

  const hovered = hoverIdx !== null ? snapshots[hoverIdx] : null;

  return (
    <div className={cn('border border-border p-4', className)}>
      <div className="flex items-center justify-between mb-3">
        <h4 className="text-xs text-text-secondary uppercase tracking-wide font-medium">
          Distribution History
        </h4>
        <div className="flex items-center gap-4 text-xs text-text-secondary">
          <span className="flex items-center gap-1.5">
            <span className="w-3 h-0.5 bg-bid inline-block" />
            {'\u03BC'} (mean)
          </span>
          <span className="flex items-center gap-1.5">
            <span className="w-3 h-0.5 inline-block" style={{ background: '#f59e0b' }} />
            {'\u03C3'} (std dev)
          </span>
        </div>
      </div>

      <div className="relative">
        <svg
          ref={svgRef}
          viewBox={`0 0 ${CHART_W} ${CHART_H}`}
          className="w-full h-auto"
          onMouseMove={handleMouseMove}
          onMouseLeave={() => setHoverIdx(null)}
        >
          {/* Grid */}
          {[0.25, 0.5, 0.75].map((f) => (
            <line
              key={f}
              x1={PAD_L}
              y1={PAD_T + PLOT_H * (1 - f)}
              x2={PAD_L + PLOT_W}
              y2={PAD_T + PLOT_H * (1 - f)}
              stroke="var(--color-border)"
              strokeWidth="0.5"
              strokeDasharray="4 4"
            />
          ))}

          {/* Mu line */}
          <path d={muPath} fill="none" stroke="var(--color-bid)" strokeWidth="2" />

          {/* Sigma line */}
          <path d={sigmaPath} fill="none" stroke="#f59e0b" strokeWidth="2" />

          {/* Hover crosshair */}
          {hovered && hoverIdx !== null && (
            <>
              <line
                x1={scaleX(new Date(hovered.capturedAt).getTime())}
                y1={PAD_T}
                x2={scaleX(new Date(hovered.capturedAt).getTime())}
                y2={PAD_T + PLOT_H}
                stroke="var(--color-border)"
                strokeWidth="1"
              />
              <circle
                cx={scaleX(new Date(hovered.capturedAt).getTime())}
                cy={scaleMuY(hovered.marketMu)}
                r="4"
                fill="var(--color-bid)"
                stroke="var(--color-bg-primary)"
                strokeWidth="2"
              />
              <circle
                cx={scaleX(new Date(hovered.capturedAt).getTime())}
                cy={scaleSigmaY(hovered.marketSigma)}
                r="4"
                fill="#f59e0b"
                stroke="var(--color-bg-primary)"
                strokeWidth="2"
              />
            </>
          )}

          {/* X-axis time labels */}
          {[0, 0.25, 0.5, 0.75, 1].map((f) => {
            const t = timeRange[0] + f * (timeRange[1] - timeRange[0]);
            const d = new Date(t);
            const label = `${d.getMonth() + 1}/${d.getDate()} ${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`;
            return (
              <text
                key={f}
                x={PAD_L + f * PLOT_W}
                y={PAD_T + PLOT_H + 20}
                textAnchor="middle"
                fill="var(--color-text-secondary)"
                fontSize="10"
                fontFamily="monospace"
              >
                {label}
              </text>
            );
          })}
        </svg>

        {/* Hover tooltip */}
        {hovered && hoverIdx !== null && (
          <div
            className="absolute pointer-events-none bg-bg-secondary border border-border px-3 py-2 text-xs"
            style={{
              left: `${((scaleX(new Date(hovered.capturedAt).getTime())) / CHART_W) * 100}%`,
              top: '4px',
              transform: hoverIdx > snapshots.length / 2 ? 'translateX(-110%)' : 'translateX(10%)',
            }}
          >
            <div className="font-mono tabular-nums text-text-primary">
              {'\u03BC'}: {hovered.marketMu.toFixed(3)}
              {outcomeUnit && <span className="text-text-muted ml-1">{outcomeUnit}</span>}
            </div>
            <div className="font-mono tabular-nums text-text-primary">
              {'\u03C3'}: {hovered.marketSigma.toFixed(3)}
            </div>
            <div className="font-mono tabular-nums text-text-secondary">
              Positions: {hovered.positionCount}
            </div>
            <div className="text-text-muted">
              {new Date(hovered.capturedAt).toLocaleString()}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
