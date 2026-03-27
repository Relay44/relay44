import type {
  DecisionContributor,
  DecisionNode,
  DecisionNodeEffect,
  DecisionNodeSourceType,
  DecisionTriggerMode,
  DecisionType,
} from "@/types";

export const DECISION_TYPE_OPTIONS: Array<{
  value: DecisionType;
  label: string;
}> = [
  { value: "timing", label: "Timing" },
  { value: "choice", label: "Choice" },
  { value: "hedge", label: "Hedge" },
  { value: "allocation", label: "Allocation" },
];

export const NODE_SOURCE_OPTIONS: Array<{
  value: DecisionNodeSourceType;
  label: string;
}> = [
  { value: "draft_market", label: "Draft node" },
  { value: "internal_market", label: "Internal market" },
  { value: "external_market", label: "External market" },
];

export const NODE_STATUS_OPTIONS = [
  { value: "draft", label: "Draft" },
  { value: "live", label: "Live" },
  { value: "inactive", label: "Inactive" },
] as const;

export const ACTION_EFFECT_OPTIONS: Array<{
  value: DecisionNodeEffect;
  label: string;
}> = [
  { value: "support", label: "Support" },
  { value: "oppose", label: "Oppose" },
  { value: "neutral", label: "Neutral" },
];

export const TRIGGER_MODE_OPTIONS: Array<{
  value: DecisionTriggerMode;
  label: string;
}> = [
  { value: "on_recommendation_gain", label: "Recommendation gain" },
  { value: "on_threshold_cross", label: "Threshold cross" },
  { value: "on_confidence_gain", label: "Confidence gain" },
];

export function humanizeDecisionValue(value: string) {
  return value.replace(/_/g, " ");
}

export function decisionTypeLabel(value: DecisionType | string) {
  return (
    DECISION_TYPE_OPTIONS.find((option) => option.value === value)?.label ||
    value
  );
}

export function triggerModeLabel(value: DecisionTriggerMode | string) {
  return (
    TRIGGER_MODE_OPTIONS.find((option) => option.value === value)?.label ||
    humanizeDecisionValue(value)
  );
}

export function nodeSourceTypeLabel(value: DecisionNodeSourceType | string) {
  return (
    NODE_SOURCE_OPTIONS.find((option) => option.value === value)?.label ||
    humanizeDecisionValue(value)
  );
}

export function recommendationLabel(value: string) {
  switch (value) {
    case "act_now":
      return "Act now";
    case "wait":
      return "Wait";
    case "insufficient_signal":
      return "Insufficient signal";
    default:
      return humanizeDecisionValue(value);
  }
}

export function defaultActionsForDecisionType(type: DecisionType): string[] {
  switch (type) {
    case "timing":
      return ["act now", "wait"];
    case "choice":
      return ["option a", "option b", "hold off"];
    case "hedge":
      return ["stay unhedged", "hedge now", "reduce exposure"];
    case "allocation":
      return ["increase allocation", "hold allocation", "reduce allocation"];
    default:
      return ["action one", "action two"];
  }
}

export function formatPercentFromBps(value: number, digits = 1) {
  return `${(value / 100).toFixed(digits)}%`;
}

export function formatBps(value: number) {
  return `${value > 0 ? "+" : ""}${value} bps`;
}

export function scoreBarWidth(scoreBps: number) {
  const clamped = Math.max(-10000, Math.min(10000, scoreBps));
  return Math.min(100, Math.abs(clamped) / 100);
}

export function contributorSummary(
  contributor: Pick<
    DecisionContributor,
    "actionLabel" | "probabilityBps" | "scoreBps"
  >,
) {
  return `${contributor.actionLabel} • ${formatPercentFromBps(contributor.probabilityBps)} • ${formatPercentFromBps(contributor.scoreBps)}`;
}

export function nodeSourceLabel(
  node: Pick<DecisionNode, "sourceType" | "sourceRef">,
) {
  if (node.sourceType === "draft_market") {
    return "Draft node";
  }
  if (!node.sourceRef) {
    return node.sourceType === "internal_market"
      ? "Internal market"
      : "External market";
  }
  return `${node.sourceType === "internal_market" ? "Internal" : "External"} • ${node.sourceRef}`;
}

export function nodeSignalLabel(
  node: Pick<DecisionNode, "lastProbabilityBps" | "lastMarketSnapshot">,
) {
  if (typeof node.lastProbabilityBps === "number") {
    return `P(yes): ${formatPercentFromBps(node.lastProbabilityBps)}`;
  }

  const status = node.lastMarketSnapshot["status"];
  if (
    typeof status === "string" &&
    (status === "unavailable" || status === "missing" || status === "invalid")
  ) {
    return "Linked market unavailable";
  }

  return "No live probability";
}
