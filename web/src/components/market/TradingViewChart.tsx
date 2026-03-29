"use client";

import { useEffect, useId, useRef } from "react";
import Link from "next/link";
import { useTheme } from "@/components/ThemeProvider";
import { cn } from "@/lib/utils";

interface TradingViewChartProps {
  symbol: string;
  sourceUrl: string;
  className?: string;
}

const WIDGET_SCRIPT_SRC =
  "https://s3.tradingview.com/external-embedding/embed-widget-advanced-chart.js";

export function TradingViewChart({
  symbol,
  sourceUrl,
  className,
}: TradingViewChartProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const widgetId = useId().replace(/:/g, "");
  const { resolvedTheme } = useTheme();

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    container.innerHTML = "";

    const widgetRoot = document.createElement("div");
    widgetRoot.className = "tradingview-widget-container";
    widgetRoot.style.height = "100%";
    widgetRoot.style.width = "100%";

    const widget = document.createElement("div");
    widget.className = "tradingview-widget-container__widget";
    widget.id = `tradingview-${widgetId}`;
    widget.style.height = "100%";
    widget.style.width = "100%";

    const script = document.createElement("script");
    script.src = WIDGET_SCRIPT_SRC;
    script.type = "text/javascript";
    script.async = true;
    script.text = JSON.stringify({
      autosize: true,
      symbol,
      interval: "60",
      timezone: "Etc/UTC",
      theme: resolvedTheme === "dark" ? "dark" : "light",
      style: "3",
      locale: "en",
      allow_symbol_change: false,
      hide_top_toolbar: true,
      hide_legend: true,
      save_image: false,
      support_host: "https://www.tradingview.com",
    });

    widgetRoot.appendChild(widget);
    widgetRoot.appendChild(script);
    container.appendChild(widgetRoot);

    return () => {
      container.innerHTML = "";
    };
  }, [resolvedTheme, symbol, widgetId]);

  return (
    <section className={cn("card p-0 overflow-hidden", className)}>
      <div className="border-b border-border px-4 py-3 sm:px-6">
        <div className="text-[0.68rem] font-mono uppercase tracking-[0.16em] text-text-muted">
          Reference chart
        </div>
        <div className="mt-1 flex flex-wrap items-center justify-between gap-3">
          <p className="text-sm text-text-secondary">
            TradingView chart for {symbol}
          </p>
          <Link
            href={sourceUrl}
            target="_blank"
            rel="noreferrer"
            className="inline-flex h-10 items-center border border-border px-4 text-[0.7rem] uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
          >
            Open on TradingView
          </Link>
        </div>
      </div>
      <div ref={containerRef} className="h-[420px] w-full bg-bg-secondary" />
    </section>
  );
}
