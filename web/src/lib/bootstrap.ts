import type { Market } from "@/types";

type BootstrapTone = "accent" | "muted" | "secondary" | "danger";

const STATUS_LABELS: Record<string, string> = {
  pending_funding: "pending funding",
  pending_authorization: "pending authorization",
  pending_launch: "launching",
  active: "active",
  paused: "paused",
  graduated: "graduated",
  error: "error",
};

const STATUS_TONES: Record<string, BootstrapTone> = {
  pending_funding: "secondary",
  pending_authorization: "secondary",
  pending_launch: "secondary",
  active: "accent",
  paused: "secondary",
  graduated: "muted",
  error: "danger",
};

export function getBootstrapStatusLabel(
  market: Pick<Market, "bootstrapStatus" | "bootstrapActive">,
): string {
  const status = market.bootstrapStatus?.trim().toLowerCase();
  if (status && STATUS_LABELS[status]) {
    return STATUS_LABELS[status];
  }
  if (market.bootstrapActive) {
    return STATUS_LABELS.active;
  }
  return STATUS_LABELS.graduated;
}

export function getBootstrapStatusTone(
  market: Pick<Market, "bootstrapStatus" | "bootstrapActive">,
): BootstrapTone {
  const status = market.bootstrapStatus?.trim().toLowerCase();
  if (status && STATUS_TONES[status]) {
    return STATUS_TONES[status];
  }
  return market.bootstrapActive ? "accent" : "muted";
}
