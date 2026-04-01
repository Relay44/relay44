"use client";

import { useState } from "react";
import { Input } from "@/components/ui/Input";
import { cn } from "@/lib/utils";
import type { OracleComparison, OracleFeedType } from "@/types";

const COMPARISON_OPTIONS: { value: OracleComparison; label: string }[] = [
  { value: "gt", label: "> greater than" },
  { value: "gte", label: ">= at least" },
  { value: "lt", label: "< less than" },
  { value: "lte", label: "<= at most" },
  { value: "eq", label: "= exactly" },
];

interface FeedPreset {
  label: string;
  address: string;
  currency: string;
  decimals: number;
}

const FEED_PRESETS: Record<string, FeedPreset[]> = {
  crypto: [
    { label: "ETH / USD", address: "0x71041dddad3595F9CEd3dCCFBe3D1F4b0a16Bb70", currency: "USD", decimals: 8 },
    { label: "BTC / USD", address: "0xCCADC697c55bbB68dc5bCdf8d3CBe83CdD4E071E", currency: "USD", decimals: 8 },
    { label: "Custom feed", address: "", currency: "USD", decimals: 8 },
  ],
  finance: [
    { label: "USDC / USD", address: "0x7e860098F58bBFC8648a4311b374B1D669a2bc9b", currency: "USD", decimals: 8 },
    { label: "Custom feed", address: "", currency: "USD", decimals: 8 },
  ],
  energy: [],
  default: [
    { label: "Custom feed", address: "", currency: "USD", decimals: 8 },
  ],
};

export interface OracleConfigValue {
  feedType: OracleFeedType;
  feedAddress: string;
  comparison: OracleComparison;
  targetValue: string;
  targetCurrency: string;
}

interface OracleConfigPanelProps {
  category: string;
  value: OracleConfigValue | null;
  onChange: (cfg: OracleConfigValue | null) => void;
}

export function OracleConfigPanel({ category, value, onChange }: OracleConfigPanelProps) {
  const presets = FEED_PRESETS[category] || FEED_PRESETS.default;
  const hasChainlinkFeeds = presets.length > 0;

  const [feedType, setFeedType] = useState<OracleFeedType>(value?.feedType || (hasChainlinkFeeds ? "chainlink" : "manual"));
  const [selectedPreset, setSelectedPreset] = useState<number>(0);
  const [customFeedAddress, setCustomFeedAddress] = useState(value?.feedAddress || "");
  const [comparison, setComparison] = useState<OracleComparison>(value?.comparison || "gt");
  const [targetValue, setTargetValue] = useState(value?.targetValue || "");
  const [targetCurrency, setTargetCurrency] = useState(value?.targetCurrency || "USD");

  const updateConfig = (updates: Partial<OracleConfigValue>) => {
    const preset = presets[selectedPreset];
    const current: OracleConfigValue = {
      feedType,
      feedAddress: preset?.address || customFeedAddress,
      comparison,
      targetValue,
      targetCurrency,
      ...updates,
    };
    onChange(current);
  };

  if (!hasChainlinkFeeds) {
    return (
      <div className="rounded-lg border border-border/50 bg-bg-secondary/50 p-4">
        <p className="text-sm text-text-secondary">
          No automated oracle feeds available for this category yet. Market will use manual resolution.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-4 rounded-lg border border-border/50 bg-bg-secondary/50 p-4">
      <div className="flex items-center gap-2 text-sm font-medium text-text-primary">
        <span className="h-2 w-2 rounded-full bg-accent" />
        Oracle Configuration
      </div>

      {/* Feed Type */}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => { setFeedType("chainlink"); updateConfig({ feedType: "chainlink" }); }}
          className={cn(
            "rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
            feedType === "chainlink"
              ? "bg-accent text-white"
              : "bg-bg-tertiary text-text-secondary hover:text-text-primary",
          )}
        >
          Chainlink Feed
        </button>
        <button
          type="button"
          onClick={() => { setFeedType("manual"); updateConfig({ feedType: "manual" }); }}
          className={cn(
            "rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
            feedType === "manual"
              ? "bg-accent text-white"
              : "bg-bg-tertiary text-text-secondary hover:text-text-primary",
          )}
        >
          Manual Resolution
        </button>
      </div>

      {feedType === "chainlink" && (
        <>
          {/* Feed Preset */}
          <div>
            <label className="mb-1 block text-xs text-text-secondary">Price Feed</label>
            <select
              value={selectedPreset}
              onChange={(e) => {
                const idx = Number(e.target.value);
                setSelectedPreset(idx);
                const preset = presets[idx];
                if (preset?.address) {
                  updateConfig({ feedAddress: preset.address, targetCurrency: preset.currency });
                  setTargetCurrency(preset.currency);
                }
              }}
              className="w-full rounded-md border border-border bg-bg-primary px-3 py-2 text-sm text-text-primary"
            >
              {presets.map((preset, idx) => (
                <option key={idx} value={idx}>{preset.label}</option>
              ))}
            </select>
          </div>

          {/* Custom Feed Address */}
          {presets[selectedPreset] && !presets[selectedPreset].address && (
            <div>
              <label className="mb-1 block text-xs text-text-secondary">Feed Contract Address</label>
              <Input
                type="text"
                placeholder="0x..."
                value={customFeedAddress}
                onChange={(e) => {
                  setCustomFeedAddress(e.target.value);
                  updateConfig({ feedAddress: e.target.value });
                }}
              />
            </div>
          )}

          {/* Comparison */}
          <div>
            <label className="mb-1 block text-xs text-text-secondary">Condition</label>
            <select
              value={comparison}
              onChange={(e) => {
                const val = e.target.value as OracleComparison;
                setComparison(val);
                updateConfig({ comparison: val });
              }}
              className="w-full rounded-md border border-border bg-bg-primary px-3 py-2 text-sm text-text-primary"
            >
              {COMPARISON_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
          </div>

          {/* Target Value */}
          <div>
            <label className="mb-1 block text-xs text-text-secondary">
              Target Price ({targetCurrency})
            </label>
            <Input
              type="number"
              step="0.01"
              placeholder="e.g. 3500.00"
              value={targetValue}
              onChange={(e) => {
                setTargetValue(e.target.value);
                updateConfig({ targetValue: e.target.value });
              }}
            />
            <p className="mt-1 text-xs text-text-tertiary">
              Market resolves YES if the price is {COMPARISON_OPTIONS.find((o) => o.value === comparison)?.label.split(" ").slice(1).join(" ")} {targetValue || "..."} {targetCurrency} at close time.
            </p>
          </div>
        </>
      )}

      {feedType === "manual" && (
        <p className="text-xs text-text-secondary">
          This market will be resolved manually by the designated resolver after the trading end time.
          The oracle keeper will not attempt automated resolution.
        </p>
      )}
    </div>
  );
}
