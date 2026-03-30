import type { Market } from "@/types";

type BootstrapTone = "accent" | "muted" | "secondary" | "danger";

const STATUS_LABELS: Record<string, string> = {
  pending_funding: "underfunded",
  pending_authorization: "needs approval",
  pending_launch: "bootstrapping",
  active: "bootstrapping",
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

const REASON_LABELS: Record<string, string> = {
  approval_missing: "operator approval missing",
  insufficient_funds: "vault balance below required depth",
  inventory_cap: "inventory cap reached",
  operator_error: "operator failed three consecutive cycles",
  graduation_pending: "ladder shutdown is pending",
  admin_paused: "paused by admin",
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

export function getBootstrapReasonLabel(reason?: string | null): string | null {
  const normalized = reason?.trim().toLowerCase();
  if (!normalized) {
    return null;
  }

  return REASON_LABELS[normalized] ?? normalized.replace(/_/g, " ");
}
