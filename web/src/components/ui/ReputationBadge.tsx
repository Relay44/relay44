import { cn } from "@/lib/utils";

interface ReputationBadgeProps {
  scoreBps?: number | null;
  confidenceBps?: number | null;
  className?: string;
}

function tierFor(scoreBps: number): { label: string; color: string } {
  if (scoreBps >= 9_000) {
    return {
      label: "S",
      color: "bg-emerald-500/20 text-emerald-400 border-emerald-500/30",
    };
  }
  if (scoreBps >= 7_500) {
    return {
      label: "A",
      color: "bg-green-500/20 text-green-400 border-green-500/30",
    };
  }
  if (scoreBps >= 5_500) {
    return {
      label: "B",
      color: "bg-blue-500/20 text-blue-400 border-blue-500/30",
    };
  }
  if (scoreBps >= 3_500) {
    return {
      label: "C",
      color: "bg-amber-500/20 text-amber-400 border-amber-500/30",
    };
  }
  return {
    label: "D",
    color: "bg-rose-500/20 text-rose-400 border-rose-500/30",
  };
}

export function ReputationBadge({
  scoreBps,
  confidenceBps,
  className,
}: ReputationBadgeProps) {
  if (scoreBps == null) {
    return (
      <span
        className={cn(
          "inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium",
          "bg-gray-500/10 text-gray-400 border-gray-500/20",
          className,
        )}
        title="No on-chain reputation yet"
      >
        —
      </span>
    );
  }

  const tier = tierFor(scoreBps);
  const scorePct = (scoreBps / 100).toFixed(1);
  const confidencePct =
    confidenceBps != null ? (confidenceBps / 100).toFixed(0) : null;
  const title =
    confidencePct != null
      ? `Score ${scorePct}% · Confidence ${confidencePct}%`
      : `Score ${scorePct}%`;

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs font-medium tabular-nums",
        tier.color,
        className,
      )}
      title={title}
    >
      <span className="font-semibold">{tier.label}</span>
      <span>{scorePct}%</span>
    </span>
  );
}
