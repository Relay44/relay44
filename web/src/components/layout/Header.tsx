"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { FormEvent, KeyboardEvent, useEffect, useState } from "react";
import { Search } from "lucide-react";
import { NotificationBell } from "@/components/notifications";
import { useBaseWallet } from "@/hooks/useBaseWallet";
import { useSolanaWallet } from "@/hooks/useSolanaWallet";
import { useRuntimeMode, useSessionState } from "@/hooks";
import { BrandLogo } from "@/components/layout/BrandLogo";
import { SidebarMenu } from "@/components/layout/SidebarMenu";
import { useToast } from "@/components/ui";
import { CHAIN_MODE } from "@/lib/constants";
import { cn } from "@/lib/utils";

const primaryLinks = [
  { href: "/markets", label: "Markets" },
  { href: "/decisions", label: "Decisions" },
  { href: "/how-it-works", label: "How it works" },
  { href: "/portfolio", label: "Portfolio" },
];

function ConnectWalletButton() {
  const baseWallet = useBaseWallet();
  const solanaWallet = useSolanaWallet();
  const { addToast } = useToast();
  const baseEnabled = CHAIN_MODE === "base" || CHAIN_MODE === "dual";
  const solanaEnabled = CHAIN_MODE === "solana" || CHAIN_MODE === "dual";
  const solanaAvailable = solanaWallet.enabled;
  const singleMode = baseEnabled !== solanaEnabled;

  const handleBaseClick = () => {
    if (baseWallet.isConnected) {
      baseWallet.disconnect();
      return;
    }
    baseWallet.connect().catch((error) => {
      console.error("Base wallet connect failed:", error);
      addToast("No supported Base wallet was detected in this browser.", "error");
    });
  };

  const handleSolanaClick = () => {
    if (solanaWallet.isConnected) {
      solanaWallet.disconnect().catch((error) => {
        console.error("Solana wallet disconnect failed:", error);
        addToast("Failed to disconnect the Solana wallet.", "error");
      });
      return;
    }
    solanaWallet.connect().catch((error) => {
      console.error("Solana wallet connect failed:", error);
      addToast("No supported Solana wallet was detected in this browser.", "error");
    });
  };

  const truncateAddress = (address: string) => {
    return `${address.slice(0, 4)}...${address.slice(-4)}`;
  };

  const installSolanaWallet = (
    <a
      href="https://phantom.app/"
      target="_blank"
      rel="noreferrer"
      className={cn(
        "h-9 px-5 text-sm font-medium inline-flex items-center",
        "border border-accent text-accent bg-transparent hover:bg-accent/10 transition-colors",
      )}
    >
      Install Phantom
    </a>
  );

  if (singleMode && baseEnabled) {
    const compactLabel =
      baseWallet.isConnected && baseWallet.address
        ? truncateAddress(baseWallet.address)
        : "Connect";

    return (
      <button
        onClick={handleBaseClick}
        className={cn(
          "h-9 max-w-[8.5rem] border border-accent bg-transparent px-3 text-xs font-medium text-accent transition-colors cursor-pointer sm:max-w-none sm:px-5 sm:text-sm",
          "hover:bg-accent/10",
        )}
      >
        <span className="truncate sm:hidden">{compactLabel}</span>
        <span className="hidden sm:inline">
          {baseWallet.isConnected && baseWallet.address
            ? truncateAddress(baseWallet.address)
            : "Connect Base"}
        </span>
      </button>
    );
  }

  if (singleMode && solanaEnabled) {
    if (!solanaAvailable) {
      return installSolanaWallet;
    }
    return (
      <button
        onClick={handleSolanaClick}
        className={cn(
          "h-9 max-w-[8.5rem] border border-accent bg-transparent px-3 text-xs font-medium text-accent transition-colors cursor-pointer sm:max-w-none sm:px-5 sm:text-sm",
          "hover:bg-accent/10",
        )}
      >
        <span className="truncate sm:hidden">
          {solanaWallet.isConnected && solanaWallet.address
            ? truncateAddress(solanaWallet.address)
            : "Connect"}
        </span>
        <span className="hidden sm:inline">
          {solanaWallet.isConnected && solanaWallet.address
            ? truncateAddress(solanaWallet.address)
            : "Connect Solana"}
        </span>
      </button>
    );
  }

  return (
    <div className="flex items-center gap-1.5 sm:gap-2">
      {baseEnabled && (
        <button
          onClick={handleBaseClick}
          className={cn(
            "h-9 border border-accent bg-transparent px-3 text-xs font-medium text-accent transition-colors",
            "hover:bg-accent/10",
          )}
        >
          {baseWallet.isConnected && baseWallet.address
            ? `Base ${truncateAddress(baseWallet.address)}`
            : "Connect Base"}
        </button>
      )}
      {solanaEnabled &&
        (solanaAvailable ? (
          <button
            onClick={handleSolanaClick}
            className={cn(
              "h-9 border border-accent bg-transparent px-3 text-xs font-medium text-accent transition-colors",
              "hover:bg-accent/10",
            )}
          >
            {solanaWallet.isConnected && solanaWallet.address
              ? `Sol ${truncateAddress(solanaWallet.address)}`
              : "Connect Solana"}
          </button>
        ) : (
          <a
            href="https://phantom.app/"
            target="_blank"
            rel="noreferrer"
            className={cn(
              "h-9 px-3 text-xs font-medium border border-border inline-flex items-center",
              "text-text-primary bg-bg-secondary hover:bg-bg-hover transition-colors",
            )}
          >
            Install Solana Wallet
          </a>
        ))}
    </div>
  );
}

export function Header() {
  const pathname = usePathname();
  const router = useRouter();
  const { capabilities } = useRuntimeMode();
  const { hasSession, sessionRestored } = useSessionState();
  const beta = capabilities?.launch?.beta ?? true;
  const [searchQuery, setSearchQuery] = useState("");

  useEffect(() => {
    const syncSearchQuery = () => {
      if (window.location.pathname === "/markets") {
        const nextQuery = new URLSearchParams(window.location.search).get("q");
        setSearchQuery(nextQuery || "");
        return;
      }

      setSearchQuery("");
    };

    syncSearchQuery();
    window.addEventListener("popstate", syncSearchQuery);
    return () => {
      window.removeEventListener("popstate", syncSearchQuery);
    };
  }, [pathname]);

  const submitSearch = () => {
    const query = searchQuery.trim();
    router.push(query ? `/markets?q=${encodeURIComponent(query)}` : "/markets");
  };

  const handleSearchSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    submitSearch();
  };

  const handleSearchKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== "Enter") {
      return;
    }

    event.preventDefault();
    submitSearch();
  };

  return (
    <header className="fixed inset-x-0 top-0 z-sticky border-b border-border bg-bg-primary text-text-primary">
      <div className="shell-frame">
        <div className="flex min-w-0 items-center gap-2 py-2 sm:h-14 sm:py-0">
          <div className="flex min-w-0 items-center gap-2.5 sm:gap-4">
            <SidebarMenu />
            <Link href="/" className="flex min-w-0 items-center group">
              <BrandLogo compact />
            </Link>
            <nav className="hidden lg:flex items-center gap-2">
              {primaryLinks.map(({ href, label }) => {
                const active =
                  pathname === href ||
                  (href !== "/" && pathname.startsWith(href));
                return (
                  <Link
                    key={href}
                    href={href}
                    className={cn(
                      "inline-flex h-9 items-center border px-3 text-xs font-medium uppercase tracking-[0.14em] transition-colors",
                      active
                        ? "border-border-hover bg-bg-secondary text-text-primary"
                        : "border-transparent text-text-muted hover:border-border hover:bg-bg-secondary hover:text-text-primary",
                    )}
                  >
                    {label}
                  </Link>
                );
              })}
            </nav>
          </div>

          <div className="hidden lg:flex min-w-0 flex-1 justify-center px-4">
            <form
              onSubmit={handleSearchSubmit}
              className="relative w-full max-w-[420px]"
            >
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text-muted" />
              <input
                type="text"
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.target.value)}
                onKeyDown={handleSearchKeyDown}
                placeholder="Search markets..."
                aria-label="Search markets"
                className={cn(
                  "w-full h-9 pl-9 pr-12 text-sm",
                  "bg-bg-secondary border border-border",
                  "text-text-primary placeholder:text-text-muted",
                  "focus:outline-none focus:border-border-hover focus:ring-1 focus:ring-accent/20",
                  "transition-colors",
                )}
              />
              <button
                type="submit"
                aria-label="Run market search"
                className={cn(
                  "absolute right-1 top-1/2 inline-flex h-7 w-7 -translate-y-1/2 items-center justify-center border border-transparent text-text-muted transition-colors",
                  "hover:border-border hover:bg-bg-primary hover:text-text-primary",
                )}
              >
                <Search className="h-4 w-4" />
              </button>
            </form>
          </div>

          <div className="ml-auto flex shrink-0 items-center gap-1.5 sm:gap-2">
            {beta ? (
              <span className="hidden sm:inline-flex h-9 items-center border border-border bg-bg-secondary px-3 text-xs font-medium uppercase tracking-[0.18em] text-text-secondary">
                Beta
              </span>
            ) : null}
            {sessionRestored && hasSession ? <NotificationBell /> : null}
            <ConnectWalletButton />
          </div>
        </div>
      </div>
    </header>
  );
}
