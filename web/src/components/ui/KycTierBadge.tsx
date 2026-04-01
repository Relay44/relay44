import { cn } from "@/lib/utils";

const TIER_CONFIG: Record<number, { label: string; color: string }> = {
  0: { label: "Unverified", color: "bg-gray-500/20 text-gray-400 border-gray-500/30" },
  1: { label: "Basic", color: "bg-blue-500/20 text-blue-400 border-blue-500/30" },
  2: { label: "Verified", color: "bg-green-500/20 text-green-400 border-green-500/30" },
  3: { label: "Institutional", color: "bg-purple-500/20 text-purple-400 border-purple-500/30" },
};

interface KycTierBadgeProps {
  tier: number;
  className?: string;
}

export function KycTierBadge({ tier, className }: KycTierBadgeProps) {
  const config = TIER_CONFIG[tier] ?? TIER_CONFIG[0];

  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium",
        config.color,
        className,
      )}
    >
      {config.label}
    </span>
  );
}
