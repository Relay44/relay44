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
import { ApiError, api } from "@/lib/api";
import { MARKET_CREATED_EVENT_ABI } from "@/lib/contracts";
import { sendPreparedTransactions } from "@/lib/evmWallet";
import { cn } from "@/lib/utils";
import type { MarketDraftOption, NewsSlide } from "@/lib/server/homeLive";
import type { Market } from "@/types";
import { OracleConfigPanel, type OracleConfigValue } from "./OracleConfigPanel";

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

const BOOTSTRAP_PRESETS = {
  tight: {
    label: "Tight",
    description: "Tighter spread for markets you expect to start with real flow.",
    levels: 4,
    spread: "2c base / 1c steps",
    cadence: "45s refresh / 120s expiry",
    exposure: "30% max one-sided exposure",
  },
  balanced: {
    label: "Balanced",
    description: "Default choice for most new markets.",
    levels: 5,
    spread: "4c base / 2c steps",
    cadence: "60s refresh / 180s expiry",
    exposure: "35% max one-sided exposure",
  },
  wide: {
    label: "Wide",
    description: "More defensive quoting for thin or noisy markets.",
    levels: 6,
    spread: "6c base / 3c steps",
    cadence: "90s refresh / 240s expiry",
    exposure: "40% max one-sided exposure",
  },
} as const;

type BootstrapPreset = keyof typeof BOOTSTRAP_PRESETS;

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

  if (message.includes("UnauthorizedResolver")) {
    return "Use your own wallet as the resolver unless you are an admin.";
  }

  return message;
}

function requiresWalletSession(error: unknown): boolean {
  return (
    error instanceof ApiError && (error.status === 401 || error.status === 403)
  );
}

async function wait(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
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
  const [submissionStage, setSubmissionStage] = useState<string | null>(null);
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
  const [liquidityMode, setLiquidityMode] = useState<
    "clob_only" | "bootstrap_hybrid"
  >("bootstrap_hybrid");
  const [initialLiquidity, setInitialLiquidity] = useState("100");
  const [initialYesPrice, setInitialYesPrice] = useState("50");
  const [bootstrapPreset, setBootstrapPreset] =
    useState<BootstrapPreset>("balanced");
  const [confirmedQuestion, setConfirmedQuestion] = useState(false);
  const [confirmedSource, setConfirmedSource] = useState(false);
  const [confirmedDeadline, setConfirmedDeadline] = useState(false);
  const [oracleConfig, setOracleConfig] = useState<OracleConfigValue | null>(null);

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
    if (liquidityMode === "bootstrap_hybrid") {
      const liquidityValue = Number(initialLiquidity);
      if (!Number.isFinite(liquidityValue) || liquidityValue < 50) {
        setError("Bootstrap liquidity requires at least 50 USDC.");
        return false;
      }

      const openingYesPrice = Number(initialYesPrice);
      if (
        !Number.isFinite(openingYesPrice) ||
        openingYesPrice < 1 ||
        openingYesPrice > 99
      ) {
        setError("Opening YES price must be between 1 and 99.");
        return false;
      }
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
    setSubmissionStage("Preparing market");

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

      let bootstrapOperator: string | undefined;
      if (liquidityMode === "bootstrap_hybrid") {
        const seedUsdc = Number(initialLiquidity);
        const requiredSeedMicrousdc = Math.floor(seedUsdc * 1_000_000);

        setSubmissionStage("Checking bootstrap funding");
        let walletBalance;
        try {
          walletBalance = await api.getWalletBalance();
        } catch (balanceError) {
          if (requiresWalletSession(balanceError)) {
            throw new Error(
              "Sign in before funding bootstrap markets from your vault.",
            );
          }
          throw balanceError;
        }

        if (walletBalance.available < requiredSeedMicrousdc) {
          const deficit = requiredSeedMicrousdc - walletBalance.available;
          setSubmissionStage("Funding vault collateral");
          let preparedDeposit;
          try {
            preparedDeposit = await api.deposit({
              amount: deficit,
              source: "wallet",
              mode: "prepare",
            });
          } catch (depositError) {
            if (requiresWalletSession(depositError)) {
              throw new Error(
                "Sign in before depositing funds for bootstrap liquidity.",
              );
            }
            throw depositError;
          }

          if (
            !preparedDeposit.intentId ||
            !preparedDeposit.preparedTransactions?.length
          ) {
            throw new Error("Deposit preparation failed");
          }

          const depositTxHash = await sendPreparedTransactions(
            walletClient,
            config,
            preparedDeposit.preparedTransactions,
            baseWallet.address as `0x${string}`,
          );
          const depositConfirmation = await api.deposit({
            amount: deficit,
            source: "wallet",
            mode: "confirm",
            intentId: preparedDeposit.intentId,
            txSignature: depositTxHash,
          });
          if (!["pending", "confirmed"].includes(depositConfirmation.status)) {
            throw new Error("Vault funding did not confirm");
          }

          setSubmissionStage("Confirming vault balance");
          let refreshedBalance = walletBalance;
          for (let attempt = 0; attempt < 5; attempt += 1) {
            refreshedBalance = await api.getWalletBalance();
            if (refreshedBalance.available >= requiredSeedMicrousdc) {
              break;
            }
            await wait(1_200);
          }

          if (refreshedBalance.available < requiredSeedMicrousdc) {
            throw new Error(
              "Vault balance is still below the bootstrap seed after deposit confirmation.",
            );
          }
        }

        setSubmissionStage("Checking operator authorization");
        const operatorStatus = await api.getBootstrapOperatorStatus(
          baseWallet.address,
        );
        bootstrapOperator = operatorStatus.operator;
        if (!operatorStatus.approved) {
          const preparedApproval = await api.prepareBaseSetManagerApproval({
            from: baseWallet.address,
            manager: operatorStatus.operator,
            approved: true,
          });
          setSubmissionStage("Authorizing bootstrap operator");
          const approvalHash = await walletClient.sendTransaction({
            account: baseWallet.address as `0x${string}`,
            to: preparedApproval.to as `0x${string}`,
            data: preparedApproval.data,
            value: BigInt(preparedApproval.value),
          });
          await waitForTransactionReceipt(config, { hash: approvalHash });
        }
      }

      setSubmissionStage("Creating market");
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
      let bootstrapWarning: string | null = null;
      try {
        setSubmissionStage(
          liquidityMode === "bootstrap_hybrid"
            ? "Registering bootstrap launch"
            : "Registering market liquidity mode",
        );
        await api.registerBaseMarketBootstrap(marketId, {
          txHash,
          liquidityMode,
          seedUsdc:
            liquidityMode === "bootstrap_hybrid" ? Number(initialLiquidity) : 0,
          initialYesBps: Math.round(Number(initialYesPrice) * 100),
          manager: bootstrapOperator,
          preset: bootstrapPreset,
        });
      } catch (bootstrapError) {
        bootstrapWarning =
          liquidityMode === "bootstrap_hybrid"
            ? `Market created, but bootstrap registration failed: ${marketCreateErrorMessage(
                bootstrapError,
              )}`
            : `Market created, but liquidity mode registration failed: ${marketCreateErrorMessage(
                bootstrapError,
              )}`;
      }

      if (resolutionSource === "oracle" && oracleConfig) {
        try {
          setSubmissionStage("Registering oracle configuration");
          await api.registerOracleMarketConfig(marketId, {
            feedType: oracleConfig.feedType,
            feedAddress: oracleConfig.feedAddress || undefined,
            comparison: oracleConfig.comparison,
            targetValue: oracleConfig.targetValue,
            targetCurrency: oracleConfig.targetCurrency,
            category,
            keeperEnabled: oracleConfig.feedType === "chainlink",
          });
        } catch (oracleError) {
          bootstrapWarning = [
            bootstrapWarning,
            `Oracle config registration failed: ${marketCreateErrorMessage(oracleError)}`,
          ]
            .filter(Boolean)
            .join(". ");
        }
      }

      onSuccess?.(marketId);

      setQuestion("");
      setDescription("");
      setCategory("");
      setResolutionSource("");
      setCustomSource("");
      setTradingEndDate("");
      setTradingEndTime("23:59");
      setLiquidityMode("bootstrap_hybrid");
      setInitialLiquidity("100");
      setInitialYesPrice("50");
      setBootstrapPreset("balanced");
      setConfirmedQuestion(false);
      setConfirmedSource(false);
      setConfirmedDeadline(false);
      setOracleConfig(null);
      setStep(1);
      setError(bootstrapWarning);
    } catch (err) {
      setError(marketCreateErrorMessage(err));
    } finally {
      setSubmissionStage(null);
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
            Resolver policy
          </p>
          <p className="mt-2 text-sm text-text-secondary">
            Publishing creates a live Base market immediately. By default, your
            connected wallet is the resolver, which means you are responsible
            for settling the final YES or NO outcome after trading closes.
          </p>
        </div>

        {loading && submissionStage ? (
          <div className="border border-accent/20 bg-accent/5 p-4">
            <p className="text-xs uppercase tracking-[0.16em] text-text-muted">
              Publish progress
            </p>
            <p className="mt-2 text-sm text-text-primary">{submissionStage}</p>
          </div>
        ) : null}

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

              {resolutionSource === "oracle" ? (
                <div className="mt-3">
                  <OracleConfigPanel
                    category={category || "default"}
                    value={oracleConfig}
                    onChange={setOracleConfig}
                  />
                </div>
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

              <div>
                <p className="text-sm text-text-secondary">Liquidity Mode</p>
                <p className="text-text-primary">
                  {liquidityMode === "bootstrap_hybrid"
                    ? "Bootstrap hybrid"
                    : "CLOB only"}
                </p>
              </div>
              {liquidityMode === "bootstrap_hybrid" ? (
                <div>
                  <p className="text-sm text-text-secondary">Bootstrap Preset</p>
                  <p className="text-text-primary">
                    {BOOTSTRAP_PRESETS[bootstrapPreset].label}
                  </p>
                </div>
              ) : null}
            </div>

            <div className="space-y-4 border border-border bg-bg-secondary p-4">
              <div>
                <p className="text-sm font-medium text-text-primary">
                  Bootstrap setup
                </p>
                <p className="mt-1 text-sm text-text-secondary">
                  Use bootstrap mode to seed thin markets with synthetic ladder
                  depth on the existing YES and NO book. CLOB only publishes the
                  market without bootstrap depth.
                </p>
              </div>

              <div className="grid gap-3 md:grid-cols-2">
                <button
                  type="button"
                  onClick={() => setLiquidityMode("bootstrap_hybrid")}
                  className={cn(
                    "border p-4 text-left transition-colors",
                    liquidityMode === "bootstrap_hybrid"
                      ? "border-accent bg-accent/10"
                      : "border-border hover:border-border-hover hover:bg-bg-primary",
                  )}
                >
                  <p className="text-xs uppercase tracking-[0.16em] text-text-muted">
                    Bootstrap hybrid
                  </p>
                  <p className="mt-2 text-sm text-text-primary">
                    Seed ladder depth, then let the book take over when organic
                    liquidity is real.
                  </p>
                </button>
                <button
                  type="button"
                  onClick={() => setLiquidityMode("clob_only")}
                  className={cn(
                    "border p-4 text-left transition-colors",
                    liquidityMode === "clob_only"
                      ? "border-accent bg-accent/10"
                      : "border-border hover:border-border-hover hover:bg-bg-primary",
                  )}
                >
                  <p className="text-xs uppercase tracking-[0.16em] text-text-muted">
                    CLOB only
                  </p>
                  <p className="mt-2 text-sm text-text-primary">
                    Publish the market with no bootstrap depth and rely on
                    organic orders from the start.
                  </p>
                </button>
              </div>

              {liquidityMode === "bootstrap_hybrid" ? (
                <>
                  <div className="grid gap-4 md:grid-cols-2">
                    <div>
                      <label className="mb-2 block text-sm font-medium text-text-secondary">
                        Initial Liquidity (USDC)
                      </label>
                      <Input
                        type="number"
                        value={initialLiquidity}
                        onChange={(e) => setInitialLiquidity(e.target.value)}
                        min="50"
                        step="10"
                      />
                      <p className="mt-1 text-xs text-text-secondary">
                        Minimum: 50 USDC. Default ladder config uses five
                        levels.
                      </p>
                    </div>

                    <div>
                      <label className="mb-2 block text-sm font-medium text-text-secondary">
                        Opening YES Price (%)
                      </label>
                      <Input
                        type="number"
                        value={initialYesPrice}
                        onChange={(e) => setInitialYesPrice(e.target.value)}
                        min="1"
                        max="99"
                        step="1"
                      />
                      <p className="mt-1 text-xs text-text-secondary">
                        Sets the starting midpoint for the bootstrap ladder.
                      </p>
                    </div>
                  </div>
                  <div className="grid gap-3 md:grid-cols-3">
                    {(Object.entries(BOOTSTRAP_PRESETS) as Array<
                      [BootstrapPreset, (typeof BOOTSTRAP_PRESETS)[BootstrapPreset]]
                    >).map(([key, preset]) => (
                      <button
                        key={key}
                        type="button"
                        onClick={() => setBootstrapPreset(key)}
                        className={cn(
                          "border p-4 text-left transition-colors",
                          bootstrapPreset === key
                            ? "border-accent bg-accent/10"
                            : "border-border hover:border-border-hover hover:bg-bg-primary",
                        )}
                      >
                        <p className="text-xs uppercase tracking-[0.16em] text-text-muted">
                          {preset.label}
                        </p>
                        <p className="mt-2 text-sm text-text-primary">
                          {preset.description}
                        </p>
                        <p className="mt-3 text-xs text-text-secondary">
                          {preset.levels} levels • {preset.spread}
                        </p>
                        <p className="mt-1 text-xs text-text-secondary">
                          {preset.cadence}
                        </p>
                        <p className="mt-1 text-xs text-text-secondary">
                          {preset.exposure}
                        </p>
                      </button>
                    ))}
                  </div>
                </>
              ) : (
                <p className="text-sm text-text-secondary">
                  Bootstrap seed capital is disabled for this market.
                </p>
              )}
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

            <div className="bg-bg-tertiary p-4">
              <p className="text-sm text-text-secondary">Creation Fee</p>
              <p className="text-xl font-semibold text-text-primary">0.5 R44</p>
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
