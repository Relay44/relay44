"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import {
  ChevronDown,
  Github,
  Settings2,
  SquareArrowOutUpRight,
  SquareDashedMousePointer,
} from "lucide-react";
import { useRuntimeMode } from "@/hooks";
import { BrandLogo } from "@/components/layout/BrandLogo";
import { ThemeToggle } from "@/components/ui/ThemeToggle";
import { cn } from "@/lib/utils";

const navLinks = [
  { href: "/", label: "Home", note: "Markets and live activity" },
  {
    href: "/how-it-works",
    label: "How it works",
    note: "Platform overview and rules",
  },
  {
    href: "/markets",
    label: "Markets",
    note: "Browse markets and prices",
  },
  {
    href: "/decisions",
    label: "Decisions",
    note: "Private decision workflows",
  },
  { href: "/agents", label: "Agents", note: "Manage market agents" },
  { href: "/identity", label: "Identity", note: "On-chain ERC-8004 agent identity" },
  { href: "/context-graph", label: "Context Graph", note: "Misinformation detection via OriginTrail DKG" },
  { href: "/leaderboard", label: "Leaderboard", note: "Top traders by performance" },
  { href: "/portfolio", label: "Portfolio", note: "Positions and open orders" },
  { href: "/wallet", label: "Wallet", note: "Balances and transfers" },
  { href: "/docs", label: "Docs", note: "Guides, API reference, and developer resources" },
];

const externalLinks = [
  {
    href: "https://x.com/relay44",
    label: "X",
    note: "Updates and announcements",
    icon: XIcon,
  },
  {
    href: "https://github.com/Relay44/relay44",
    label: "GitHub",
    note: "Source code and issues",
    icon: Github,
  },
];

function XIcon(props: React.ComponentProps<"svg">) {
  return (
    <svg aria-hidden viewBox="0 0 24 24" fill="currentColor" {...props}>
      <path d="M18.901 2H22l-6.77 7.737L23.2 22h-6.244l-4.89-7.467L5.53 22H2.43l7.24-8.275L2 2h6.402l4.42 6.75L18.9 2Zm-1.087 18.127h1.717L7.47 3.777H5.628l12.186 16.35Z" />
    </svg>
  );
}

function MenuGlyph() {
  return (
    <span className="flex h-4 w-4 flex-col justify-between" aria-hidden>
      <span className="block h-px w-full bg-current" />
      <span className="block h-px w-full bg-current" />
      <span className="block h-px w-full bg-current" />
    </span>
  );
}

export function SidebarMenu() {
  const pathname = usePathname();
  const { readOnly } = useRuntimeMode();
  const [open, setOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    setOpen(false);
  }, [pathname]);

  useEffect(() => {
    if (typeof document === "undefined") {
      return;
    }

    const previous = document.body.style.overflow;
    document.body.style.overflow = open ? "hidden" : previous;
    return () => {
      document.body.style.overflow = previous;
    };
  }, [open]);

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        className={cn(
          "inline-flex h-9 w-9 shrink-0 items-center justify-center border border-border text-text-primary transition-colors",
          open
            ? "pointer-events-none opacity-0"
            : "bg-bg-secondary hover:bg-bg-hover",
        )}
        aria-label="Open navigation menu"
        aria-expanded={open}
      >
        <MenuGlyph />
      </button>

      {open ? (
        <div className="fixed inset-0 z-[65] transition-opacity duration-normal pointer-events-auto opacity-100">
        <button
          type="button"
          onClick={() => setOpen(false)}
          className="absolute inset-0 bg-black/35"
          aria-label="Close navigation menu"
        />

        <aside
          className={cn(
            "absolute right-0 top-0 flex h-full w-[min(360px,92vw)] flex-col border-l border-border bg-bg-primary transition-transform duration-normal",
            open ? "translate-x-0" : "translate-x-full",
          )}
        >
          <div className="border-b border-border px-5 pb-5 pt-6">
            <p className="text-[11px] uppercase tracking-[0.22em] text-text-muted">
              navigation
            </p>
            <div className="mt-3">
              <BrandLogo />
            </div>
            {readOnly ? (
              <div className="mt-4 inline-flex border border-accent/30 bg-accent/10 px-3 py-1 text-[11px] uppercase tracking-[0.18em] text-accent">
                Read only
              </div>
            ) : null}
          </div>

          <nav className="flex-1 overflow-y-auto px-5 py-5">
            <div className="space-y-2">
              {navLinks.map(({ href, label, note }) => {
                const active =
                  pathname === href ||
                  (href !== "/" && pathname.startsWith(href));
                return (
                  <Link
                    key={href}
                    href={href}
                    className={cn(
                      "block border px-4 py-3 transition-colors",
                      active
                        ? "border-accent bg-accent/10 text-text-primary"
                        : "border-border text-text-secondary hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary",
                    )}
                  >
                    <div className="flex items-center justify-between gap-4">
                      <span className="text-sm font-medium uppercase tracking-[0.12em]">
                        {label}
                      </span>
                      <SquareDashedMousePointer className="h-4 w-4 opacity-40" />
                    </div>
                    <p className="mt-2 text-xs uppercase tracking-[0.14em] text-text-muted">
                      {note}
                    </p>
                  </Link>
                );
              })}
            </div>

            <div className="mt-6 border-t border-border pt-6">
              <p className="text-[11px] uppercase tracking-[0.22em] text-text-muted">
                external
              </p>
              <div className="mt-3 space-y-2">
                {externalLinks.map(({ href, label, note, icon: Icon }) => (
                  <a
                    key={href}
                    href={href}
                    target="_blank"
                    rel="noreferrer"
                    className="block border border-border px-4 py-3 text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
                  >
                    <div className="flex items-center justify-between gap-4">
                      <span className="inline-flex items-center gap-3 text-sm font-medium uppercase tracking-[0.12em]">
                        <Icon className="h-4 w-4" />
                        {label}
                      </span>
                      <SquareArrowOutUpRight className="h-4 w-4 opacity-40" />
                    </div>
                    <p className="mt-2 text-xs uppercase tracking-[0.14em] text-text-muted">
                      {note}
                    </p>
                  </a>
                ))}
              </div>
            </div>
          </nav>

          <div className="border-t border-border px-5 py-5">
            <button
              type="button"
              onClick={() => setSettingsOpen((current) => !current)}
              className="flex w-full items-center justify-between border border-border px-4 py-3 text-left text-sm font-medium uppercase tracking-[0.14em] text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
              aria-expanded={settingsOpen}
            >
              <span className="inline-flex items-center gap-2">
                <Settings2 className="h-4 w-4" />
                Settings
              </span>
              <ChevronDown
                className={cn(
                  "h-4 w-4 transition-transform",
                  settingsOpen && "rotate-180",
                )}
              />
            </button>

            <div
              className={cn(
                "overflow-hidden transition-[max-height,opacity,margin] duration-normal",
                settingsOpen
                  ? "mt-3 max-h-40 opacity-100"
                  : "max-h-0 opacity-0",
              )}
            >
              <div className="border border-border bg-bg-secondary px-4 py-3">
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <p className="text-xs uppercase tracking-[0.16em] text-text-muted">
                      Appearance
                    </p>
                    <p className="mt-1 text-sm text-text-primary">
                      Dark / light switch
                    </p>
                  </div>
                  <ThemeToggle iconless className="bg-transparent" />
                </div>
              </div>
            </div>
          </div>
        </aside>
        </div>
      ) : null}
    </>
  );
}
