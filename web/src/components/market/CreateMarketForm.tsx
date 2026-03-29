"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { waitForTransactionReceipt } from "@wagmi/core";
import { parseEventLogs } from "viem";
import { useConfig, useWalletClient } from "wagmi";
import { useMarkets, useRuntimeMode } from "@/hooks";
import { useBaseWallet } from "@/hooks/useBaseWallet";
import { ReadOnlyNotice } from "@/components/runtime/ReadOnlyNotice";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { api } from "@/lib/api";
import { MARKET_CREATED_EVENT_ABI } from "@/lib/contracts";
import { cn } from "@/lib/utils";
import type { MarketDraftOption, NewsSlide } from "@/lib/server/homeLive";
import type { Market } from "@/types";

interface CreateMarketFormProps {
  onSuccess?: (marketId: string) => void;
  draftSlide?: NewsSlide | null;
  initialDraftId?: string;
  initialQuestion?: string;
  initialDescription?: string;
  initialCategory?: string;
  initialResolutionSource?: string;
  initialCustomSource?: string;
  initialTradingEnd?: string;
}

interface DraftMatch {
  market: Market;
  exact: boolean;
  score: number;
}

const CATEGORIES = [
  { id: "crypto", label: "Crypto", icon: "₿" },
  { id: "politics", label: "Politics", icon: "🏛" },
  { id: "sports", label: "Sports", icon: "⚽" },
  { id: "tech", label: "Technology", icon: "💻" },
  { id: "entertainment", label: "Entertainment", icon: "🎬" },
  { id: "science", label: "Science", icon: "🔬" },
  { id: "finance", label: "Finance", icon: "📈" },
  { id: "other", label: "Other", icon: "📌" },
];

const RESOLUTION_SOURCES = [
  {
    id: "official",
    label: "Official Source",
    description: "Government, company announcements",
  },
  {
    id: "oracle",
    label: "Price Oracle",
    description: "For price-based markets",
  },
  {
    id: "news",
    label: "News Outlets",
    description: "Major news organizations",
  },
  { id: "custom", label: "Custom", description: "Specify your own source" },
] as const;

function splitTradingEnd(value?: string): { date: string; time: string } {
  if (!value) {
    return { date: "", time: "23:59" };
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return { date: "", time: "23:59" };
  }

  const iso = parsed.toISOString();
  return {
    date: iso.slice(0, 10),
    time: iso.slice(11, 16),
  };
}

function buildTradingEnd(date: string, time: string): Date {
  return new Date(`${date}T${time}:00Z`);
}

function normalizeQuestion(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

function questionTokens(value: string): string[] {
  return normalizeQuestion(value)
    .split(" ")
    .filter(
      (token) =>
        token.length > 3 &&
        ![
          "will",
          "this",
          "that",
          "with",
          "from",
          "into",
          "they",
          "have",
        ].includes(token),
    );
}

function findDuplicateMarkets(
  question: string,
  markets: Market[],
): DraftMatch[] {
  const normalized = normalizeQuestion(question);
  if (!normalized) {
    return [];
  }

  const tokens = questionTokens(question);

  return markets
    .map((market) => {
      const marketNormalized = normalizeQuestion(market.question);
      const exact = marketNormalized === normalized;
      const marketTokens = questionTokens(market.question);
      const overlapCount = tokens.filter((token) =>
        marketTokens.includes(token),
      ).length;
      const score = tokens.length > 0 ? overlapCount / tokens.length : 0;

      return {
        market,
        exact,
        score,
      };
    })
    .filter(
      ({ exact, score }) => exact || (score >= 0.55 && question.length > 20),
    )
    .sort(
      (left, right) =>
        Number(right.exact) - Number(left.exact) || right.score - left.score,
    )
    .slice(0, 3);
}

function formatTradingEndLabel(date: string, time: string): string {
  return (
    buildTradingEnd(date, time).toLocaleString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
      timeZone: "UTC",
    }) + " UTC"
  );
}

function marketCreateErrorMessage(error: unknown): string {
  const message =
    error instanceof Error ? error.message : "Failed to create market";

  if (
    message.includes("AccessControlUnauthorizedAccount") ||
    message.includes("MARKET_CREATOR_ROLE")
  ) {
    return "This wallet is not allowed to publish internal relay44 markets.";
  }

  return message;
}

export function CreateMarketForm({
  onSuccess,
  draftSlide,
  initialDraftId,
  initialQuestion,
  initialDescription,
  initialCategory,
  initialResolutionSource,
  initialCustomSource,
  initialTradingEnd,
}: CreateMarketFormProps) {
  const baseWallet = useBaseWallet();
  const config = useConfig();
  const { data: walletClient } = useWalletClient();
  const { readOnly } = useRuntimeMode();
  const { date: initialTradingEndDate, time: initialTradingEndTime } =
    splitTradingEnd(initialTradingEnd);
  const draftOptions = useMemo(
    () => draftSlide?.marketDrafts ?? [],
    [draftSlide],
  );

  const [step, setStep] = useState(1);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedDraftId, setSelectedDraftId] = useState(
    initialDraftId || draftOptions[0]?.id || "",
  );

  const [question, setQuestion] = useState(initialQuestion || "");
  const [description, setDescription] = useState(initialDescription || "");
  const [category, setCategory] = useState(initialCategory || "");
  const [resolutionSource, setResolutionSource] = useState(
    initialResolutionSource || "",
  );
  const [customSource, setCustomSource] = useState(initialCustomSource || "");
  const [tradingEndDate, setTradingEndDate] = useState(initialTradingEndDate);
  const [tradingEndTime, setTradingEndTime] = useState(initialTradingEndTime);
  const [initialLiquidity, setInitialLiquidity] = useState("100");
  const [confirmedQuestion, setConfirmedQuestion] = useState(false);
  const [confirmedSource, setConfirmedSource] = useState(false);
  const [confirmedDeadline, setConfirmedDeadline] = useState(false);

  const activeDraft = useMemo(() => {
    if (!draftOptions.length) {
      return null;
    }

    return (
      draftOptions.find((draft) => draft.id === selectedDraftId) ||
      draftOptions[0]
    );
  }, [draftOptions, selectedDraftId]);

  const { data: marketsData } = useMarkets({
    limit: 150,
    source: "all",
    tradable: "all",
    sort: "volume",
  });

  const duplicateMatches = useMemo(
    () => findDuplicateMarkets(question, marketsData?.data || []),
    [question, marketsData?.data],
  );
  const exactDuplicate = duplicateMatches.find((match) => match.exact) || null;

  useEffect(() => {
    if (!draftOptions.length) {
      return;
    }

    if (!selectedDraftId) {
      setSelectedDraftId(initialDraftId || draftOptions[0].id);
    }
  }, [draftOptions, initialDraftId, selectedDraftId]);

  useEffect(() => {
    if (!activeDraft) {
      return;
    }

    const nextTradingEnd = splitTradingEnd(activeDraft.tradingEnd);
    setQuestion(activeDraft.question);
    setDescription(activeDraft.description);
    setCategory(activeDraft.category);
    setResolutionSource(activeDraft.resolutionSource);
    setCustomSource(activeDraft.customSource || "");
    setTradingEndDate(nextTradingEnd.date);
    setTradingEndTime(nextTradingEnd.time);
    setConfirmedQuestion(false);
    setConfirmedSource(false);
    setConfirmedDeadline(false);
  }, [activeDraft]);

  const validateStep1 = () => {
    if (!question.trim()) {
      setError("Question is required");
      return false;
    }
    if (question.length < 10) {
      setError("Question must be at least 10 characters");
      return false;
    }
    if (question.length > 200) {
      setError("Question must be less than 200 characters");
      return false;
    }
    if (!question.endsWith("?")) {
      setError("Question must end with a question mark");
      return false;
    }
    if (exactDuplicate) {
      setError(
        "This draft matches an existing market. Pick another angle or edit the wording.",
      );
      return false;
    }
    setError(null);
    return true;
  };

  const validateStep2 = () => {
    if (!category) {
      setError("Please select a category");
      return false;
    }
    setError(null);
    return true;
  };

  const validateStep3 = () => {
    if (!resolutionSource) {
      setError("Please select a resolution source");
      return false;
    }
    if (resolutionSource === "custom" && !customSource.trim()) {
      setError("Please specify the resolution source");
      return false;
    }
    if (!tradingEndDate) {
      setError("Please set a trading end date");
      return false;
    }
    const endDate = buildTradingEnd(tradingEndDate, tradingEndTime);
    if (endDate <= new Date()) {
      setError("Trading end date must be in the future");
      return false;
    }
    setError(null);
    return true;
  };

  const validateReview = () => {
    if (!confirmedQuestion || !confirmedSource || !confirmedDeadline) {
      setError("Confirm wording, source, and deadline before publishing.");
      return false;
    }
    if (exactDuplicate) {
      setError(
        "This draft matches an existing market. Pick another angle or edit the wording.",
      );
      return false;
    }
    setError(null);
    return true;
  };

  const handleNextStep = () => {
    if (step === 1 && validateStep1()) {
      setStep(2);
    } else if (step === 2 && validateStep2()) {
      setStep(3);
    } else if (step === 3 && validateStep3()) {
      setStep(4);
    }
  };

  const handlePrevStep = () => {
    setError(null);
    setStep((current) => Math.max(1, current - 1));
  };

  const handleSelectDraft = (draft: MarketDraftOption) => {
    setSelectedDraftId(draft.id);
    setStep(1);
    setError(null);
  };

  const handleSubmit = async () => {
    if (!validateReview()) {
      return;
    }

    if (!baseWallet.isConnected || !baseWallet.address) {
      setError("Please connect your wallet");
      return;
    }

    setLoading(true);
    setError(null);

    try {
      await baseWallet.ensureBaseChain();
      if (!walletClient) {
        throw new Error("Wallet client unavailable");
      }

      const closeTimeSeconds = BigInt(
        Math.floor(
          buildTradingEnd(tradingEndDate, tradingEndTime).getTime() / 1000,
        ),
      );
      if (closeTimeSeconds <= BigInt(Math.floor(Date.now() / 1000))) {
        throw new Error("Trading end date must be in the future");
      }

      const resolver = baseWallet.address as `0x${string}`;
      const prepared = await api.prepareBaseCreateMarket({
        from: baseWallet.address,
        question: question.trim(),
        description: description.trim(),
        category,
        resolutionSource:
          resolutionSource === "custom"
            ? customSource.trim()
            : resolutionSource,
        closeTime: Number(closeTimeSeconds),
        resolver,
      });
      const txHash = await walletClient.sendTransaction({
        account: baseWallet.address as `0x${string}`,
        to: prepared.to as `0x${string}`,
        data: prepared.data,
        value: BigInt(prepared.value),
      });

      const receipt = await waitForTransactionReceipt(config, { hash: txHash });
      const [event] = parseEventLogs({
        abi: MARKET_CREATED_EVENT_ABI,
        eventName: "MarketCreated",
        logs: receipt.logs,
      });

      const marketId = event?.args.marketId?.toString() || "";
      if (!marketId) {
        throw new Error("Market created but market id was not emitted");
      }
      onSuccess?.(marketId);

      setQuestion("");
      setDescription("");
      setCategory("");
      setResolutionSource("");
      setCustomSource("");
      setTradingEndDate("");
      setTradingEndTime("23:59");
      setInitialLiquidity("100");
      setConfirmedQuestion(false);
      setConfirmedSource(false);
      setConfirmedDeadline(false);
      setStep(1);
    } catch (err) {
      setError(marketCreateErrorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  const tomorrow = new Date();
  tomorrow.setUTCDate(tomorrow.getUTCDate() + 1);
  const minDate = tomorrow.toISOString().split("T")[0];

  if (readOnly) {
    return (
      <ReadOnlyNotice
        title="Market creation is currently unavailable"
        body="Market discovery stays live, but market creation is unavailable in this environment."
        actionHref="/markets"
        actionLabel="Browse markets"
      />
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{draftSlide ? "Draft Market" : "Create Market"}</CardTitle>
        <p className="mt-2 text-sm text-text-secondary">
          {draftSlide
            ? "Review the draft, confirm the resolution logic, and only then publish."
            : "Define a clear, resolvable question before publishing."}
        </p>
        <div className="mt-4 flex gap-2">
          {[1, 2, 3, 4].map((s) => (
            <div
              key={s}
              className={cn(
                "h-1 flex-1 transition-colors",
                s <= step ? "bg-accent" : "bg-bg-tertiary",
              )}
            />
          ))}
        </div>
      </CardHeader>

      <CardContent className="space-y-6">
        <div className="border border-border bg-bg-secondary p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-text-muted">
            Live contract policy
          </p>
          <p className="mt-2 text-sm text-text-secondary">
            Publishing internal relay44 markets is limited to operator wallets
            on the live MarketCore contract. Unauthorized wallets can review
            drafts here, but publish will fail onchain.
          </p>
        </div>

        {draftSlide && draftOptions.length > 0 ? (
          <div className="space-y-4 border border-accent/20 bg-accent/5 p-4">
            <div className="flex flex-wrap items-center gap-3">
              <span className="border border-accent/30 bg-accent/10 px-3 py-1 text-[11px] uppercase tracking-[0.18em] text-accent">
                Drafted from live news
              </span>
              <span className="text-[11px] uppercase tracking-[0.16em] text-text-muted">
                {draftSlide.kicker}
              </span>
            </div>

            <div>
              <h3 className="text-lg font-semibold text-text-primary">
                {draftSlide.headline}
              </h3>
              <p className="mt-2 text-sm text-text-secondary">
                Pick the angle that is most objective and easiest to resolve.
                You still have to confirm the wording, deadline, and source
                before publish.
              </p>
            </div>

            <div className="grid gap-3 md:grid-cols-3">
              {draftOptions.map((draft) => (
                <button
                  key={draft.id}
                  type="button"
                  onClick={() => handleSelectDraft(draft)}
                  className={cn(
                    "border p-4 text-left transition-colors",
                    selectedDraftId === draft.id
                      ? "border-accent bg-accent/10"
                      : "border-border hover:border-border-hover hover:bg-bg-secondary",
                  )}
                >
                  <p className="text-xs uppercase tracking-[0.16em] text-text-muted">
                    {draft.label}
                  </p>
                  <p className="mt-2 text-sm font-medium text-text-primary">
                    {draft.summary}
                  </p>
                  <p className="mt-3 text-xs text-text-secondary">
                    {draft.question}
                  </p>
                </button>
              ))}
            </div>

            <div className="flex flex-wrap items-center gap-3 text-xs uppercase tracking-[0.16em] text-text-muted">
              <span>Source story</span>
              <a
                href={draftSlide.sourceUrl}
                target="_blank"
                rel="noreferrer"
                className="text-accent transition-colors hover:text-accent-hover"
              >
                Open original coverage
              </a>
            </div>
          </div>
        ) : null}

        {step === 1 && (
          <div className="space-y-4">
            <div className="border border-border bg-bg-secondary p-4">
              <p className="text-xs uppercase tracking-[0.16em] text-text-muted">
                Question checklist
              </p>
              <ul className="mt-3 space-y-2 text-sm leading-6 text-text-secondary">
                <li>Write one objective yes or no outcome.</li>
                <li>
                  Include the event boundary or date inside the question itself.
                </li>
                <li>
                  Avoid subjective wording or duplicate markets that split
                  liquidity.
                </li>
              </ul>
            </div>

            <div>
              <label className="mb-2 block text-sm font-medium text-text-secondary">
                Market Question
              </label>
              <Input
                value={question}
                onChange={(e) => setQuestion(e.target.value)}
                placeholder="Will Bitcoin reach $100,000 by December 2025?"
                className="text-lg"
              />
              <div className="mt-2 flex items-center justify-between gap-3">
                <p className="text-xs text-text-secondary">
                  {question.length}/200 characters
                </p>
                {activeDraft ? (
                  <p className="text-xs uppercase tracking-[0.14em] text-text-muted">
                    Draft angle: {activeDraft.label}
                  </p>
                ) : null}
              </div>
            </div>

            <div>
              <label className="mb-2 block text-sm font-medium text-text-secondary">
                Description (optional)
              </label>
              <textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Add context, resolution criteria, or relevant links..."
                className="h-28 w-full resize-none border border-border bg-bg-secondary px-3 py-2 text-text-primary placeholder:text-text-secondary focus:outline-none focus:ring-2 focus:ring-accent"
              />
            </div>

            {exactDuplicate ? (
              <div className="border border-ask/30 bg-ask/10 p-4">
                <p className="text-sm font-medium text-ask">
                  Duplicate market detected
                </p>
                <p className="mt-1 text-sm text-text-primary">
                  {exactDuplicate.market.question}
                </p>
                <a
                  href={`/markets/${exactDuplicate.market.id}`}
                  className="mt-3 inline-flex text-sm text-accent transition-colors hover:text-accent-hover"
                >
                  Inspect existing market
                </a>
              </div>
            ) : duplicateMatches.length > 0 ? (
              <div className="border border-border bg-bg-secondary p-4">
                <p className="text-sm font-medium text-text-primary">
                  Possible related markets
                </p>
                <div className="mt-3 space-y-2">
                  {duplicateMatches.map((match) => (
                    <a
                      key={match.market.id}
                      href={`/markets/${match.market.id}`}
                      className="block border border-border px-3 py-2 text-sm text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-primary hover:text-text-primary"
                    >
                      {match.market.question}
                    </a>
                  ))}
                </div>
              </div>
            ) : null}
          </div>
        )}

        {step === 2 && (
          <div className="space-y-4">
            <label className="mb-2 block text-sm font-medium text-text-secondary">
              Category
            </label>
            <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
              {CATEGORIES.map((cat) => (
                <button
                  key={cat.id}
                  type="button"
                  onClick={() => setCategory(cat.id)}
                  className={cn(
                    "border p-4 text-center transition-all duration-fast cursor-pointer",
                    category === cat.id
                      ? "border-accent bg-accent-muted"
                      : "border-border hover:border-border-hover",
                  )}
                >
                  <span className="mb-1 block text-2xl">{cat.icon}</span>
                  <span className="text-sm text-text-primary">{cat.label}</span>
                </button>
              ))}
            </div>
          </div>
        )}

        {step === 3 && (
          <div className="space-y-6">
            <div className="grid gap-4 md:grid-cols-2">
              <div className="border border-border bg-bg-secondary p-4">
                <p className="text-xs uppercase tracking-[0.16em] text-text-muted">
                  Resolution rules
                </p>
                <ul className="mt-3 space-y-2 text-sm leading-6 text-text-secondary">
                  <li>
                    Use a source that can settle the outcome without
                    interpretation.
                  </li>
                  <li>
                    Custom sources should point to one authoritative URL or
                    document.
                  </li>
                  <li>
                    Pick the narrowest source that resolves the exact wording
                    you wrote.
                  </li>
                </ul>
              </div>

              <div className="border border-border bg-bg-secondary p-4">
                <p className="text-xs uppercase tracking-[0.16em] text-text-muted">
                  Deadline rules
                </p>
                <ul className="mt-3 space-y-2 text-sm leading-6 text-text-secondary">
                  <li>
                    Trading should end after users have time to discover the
                    market.
                  </li>
                  <li>
                    The deadline should still leave a clean window for
                    resolution and payout.
                  </li>
                  <li>
                    All times here are UTC, so check the date before you
                    publish.
                  </li>
                </ul>
              </div>
            </div>

            <div>
              <label className="mb-2 block text-sm font-medium text-text-secondary">
                Resolution Source
              </label>
              <div className="space-y-2">
                {RESOLUTION_SOURCES.map((source) => (
                  <button
                    key={source.id}
                    type="button"
                    onClick={() => setResolutionSource(source.id)}
                    className={cn(
                      "w-full border p-4 text-left transition-all duration-fast cursor-pointer",
                      resolutionSource === source.id
                        ? "border-accent bg-accent-muted"
                        : "border-border hover:border-border-hover",
                    )}
                  >
                    <span className="font-medium text-text-primary">
                      {source.label}
                    </span>
                    <span className="mt-1 block text-sm text-text-secondary">
                      {source.description}
                    </span>
                  </button>
                ))}
              </div>

              {resolutionSource === "custom" ? (
                <Input
                  value={customSource}
                  onChange={(e) => setCustomSource(e.target.value)}
                  placeholder="Specify the resolution source URL or description"
                  className="mt-3"
                />
              ) : null}
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="mb-2 block text-sm font-medium text-text-secondary">
                  Trading End Date
                </label>
                <Input
                  type="date"
                  value={tradingEndDate}
                  onChange={(e) => setTradingEndDate(e.target.value)}
                  min={minDate}
                />
              </div>
              <div>
                <label className="mb-2 block text-sm font-medium text-text-secondary">
                  Trading End Time (UTC)
                </label>
                <Input
                  type="time"
                  value={tradingEndTime}
                  onChange={(e) => setTradingEndTime(e.target.value)}
                />
              </div>
            </div>
          </div>
        )}

        {step === 4 && (
          <div className="space-y-4">
            <h3 className="font-medium text-text-primary">Review Draft</h3>

            <div className="border border-accent/20 bg-accent/5 p-4">
              <p className="text-xs uppercase tracking-[0.16em] text-accent">
                Publish check
              </p>
              <p className="mt-2 text-sm leading-6 text-text-secondary">
                Publishing sends an onchain create-market transaction. Verify
                the wording, source, and deadline now, because traders will rely
                on this exact market definition.
              </p>
            </div>

            <div className="space-y-3 bg-bg-secondary p-4">
              <div>
                <p className="text-sm text-text-secondary">Question</p>
                <p className="text-text-primary">{question}</p>
              </div>

              {description ? (
                <div>
                  <p className="text-sm text-text-secondary">Description</p>
                  <p className="text-sm text-text-primary">{description}</p>
                </div>
              ) : null}

              <div className="grid grid-cols-2 gap-4">
                <div>
                  <p className="text-sm text-text-secondary">Category</p>
                  <p className="text-text-primary">
                    {CATEGORIES.find((item) => item.id === category)?.label}
                  </p>
                </div>
                <div>
                  <p className="text-sm text-text-secondary">Resolution</p>
                  <p className="text-text-primary">
                    {resolutionSource === "custom"
                      ? customSource
                      : RESOLUTION_SOURCES.find(
                          (item) => item.id === resolutionSource,
                        )?.label}
                  </p>
                </div>
              </div>

              <div>
                <p className="text-sm text-text-secondary">Trading Ends</p>
                <p className="text-text-primary">
                  {formatTradingEndLabel(tradingEndDate, tradingEndTime)}
                </p>
              </div>
            </div>

            <div className="space-y-3 border border-border bg-bg-primary p-4">
              <p className="text-sm font-medium text-text-primary">
                Required confirmations
              </p>
              <label className="flex items-start gap-3 text-sm text-text-secondary">
                <input
                  type="checkbox"
                  checked={confirmedQuestion}
                  onChange={(e) => setConfirmedQuestion(e.target.checked)}
                  className="mt-1"
                />
                <span>
                  I reviewed the wording and it resolves on an objective yes or
                  no outcome.
                </span>
              </label>
              <label className="flex items-start gap-3 text-sm text-text-secondary">
                <input
                  type="checkbox"
                  checked={confirmedSource}
                  onChange={(e) => setConfirmedSource(e.target.checked)}
                  className="mt-1"
                />
                <span>
                  I confirmed the primary resolution source and it is specific
                  enough to audit.
                </span>
              </label>
              <label className="flex items-start gap-3 text-sm text-text-secondary">
                <input
                  type="checkbox"
                  checked={confirmedDeadline}
                  onChange={(e) => setConfirmedDeadline(e.target.checked)}
                  className="mt-1"
                />
                <span>
                  I confirmed the trading deadline and it matches the market
                  question.
                </span>
              </label>
            </div>

            <div>
              <label className="mb-2 block text-sm font-medium text-text-secondary">
                Initial Liquidity (USDC)
              </label>
              <Input
                type="number"
                value={initialLiquidity}
                onChange={(e) => setInitialLiquidity(e.target.value)}
                min="10"
                step="10"
              />
              <p className="mt-1 text-xs text-text-secondary">
                Minimum: 10 USDC. Higher liquidity attracts more traders.
              </p>
            </div>

            <div className="bg-bg-tertiary p-4">
              <p className="text-sm text-text-secondary">Creation Fee</p>
              <p className="text-xl font-semibold text-text-primary">
                0.5 R44
              </p>
            </div>
          </div>
        )}

        {error ? (
          <div className="border border-ask/20 bg-ask/10 p-3">
            <p className="text-sm text-ask">{error}</p>
          </div>
        ) : null}

        {step === 4 && !baseWallet.isConnected ? (
          <div className="border border-border bg-bg-secondary p-4 text-sm leading-6 text-text-secondary">
            Connect your Base wallet from the header before you publish. If you
            need the launch rules or settlement checklist first, review{" "}
            <Link
              href="/how-it-works"
              className="text-accent transition-colors hover:text-accent-hover"
            >
              how relay44 works
            </Link>
            .
          </div>
        ) : null}

        <div className="flex justify-between pt-4">
          {step > 1 ? (
            <Button variant="secondary" onClick={handlePrevStep}>
              Back
            </Button>
          ) : (
            <div />
          )}

          {step < 4 ? (
            <Button variant="primary" onClick={handleNextStep}>
              Continue
            </Button>
          ) : (
            <Button
              variant="primary"
              onClick={handleSubmit}
              loading={loading}
              disabled={
                !baseWallet.isConnected ||
                !confirmedQuestion ||
                !confirmedSource ||
                !confirmedDeadline
              }
            >
              {baseWallet.isConnected ? "Publish Market" : "Connect Wallet"}
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
