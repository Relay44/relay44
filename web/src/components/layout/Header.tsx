"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { FormEvent, KeyboardEvent, useEffect, useState } from "react";
import { Search } from "lucide-react";
import { NotificationBell } from "@/components/notifications";
import { useBaseWallet } from "@/hooks/useBaseWallet";
import { useSolanaWallet } from "@/hooks/useSolanaWallet";
import { useBasename } from "@/hooks/useBasename";
import { useRuntimeMode, useSessionState } from "@/hooks";
import { BrandLogo } from "@/components/layout/BrandLogo";
import { SidebarMenu } from "@/components/layout/SidebarMenu";
import { useToast } from "@/components/ui";
import { CHAIN_MODE } from "@/lib/constants";
import { cn } from "@/lib/utils";

const primaryLinks = [
  { href: "/markets", label: "Markets", tourId: "tour-nav-markets" },
  { href: "/distribution", label: "Distribution" },
  { href: "/decisions", label: "Decisions", tourId: "tour-nav-decisions" },
  { href: "/agents", label: "Swarm", tourId: "tour-nav-agents" },
  { href: "/portfolio", label: "Portfolio", tourId: "tour-nav-portfolio" },
];

function ConnectWalletButton() {
  const baseWallet = useBaseWallet();
  const solanaWallet = useSolanaWallet();
  const { basename } = useBasename(baseWallet.address);
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
  const displayBaseAddress = (address: string) => basename || truncateAddress(address);
  const walletButtonClass =
    'inline-flex h-10 items-center border border-border bg-transparent px-4 text-[0.7rem] font-mono text-text-primary transition-colors hover:bg-bg-hover';

  const installSolanaWallet = (
    <a
      href="https://phantom.app/"
      target="_blank"
      rel="noreferrer"
      className={walletButtonClass}
    >
      Install Phantom
    </a>
  );

  if (singleMode && baseEnabled) {
    const compactLabel =
      baseWallet.isConnected && baseWallet.address
        ? displayBaseAddress(baseWallet.address)
        : "Connect";

    return (
      <button
        onClick={handleBaseClick}
        aria-label={
          baseWallet.isConnected && baseWallet.address
            ? `Connected wallet ${displayBaseAddress(baseWallet.address)}`
            : "Connect Wallet"
        }
        className="inline-flex h-10 max-w-[8.5rem] items-center border border-border bg-transparent px-3 text-[0.7rem] font-mono text-text-primary transition-colors cursor-pointer hover:bg-bg-hover sm:max-w-none sm:px-4"
      >
        <span className="truncate sm:hidden">{compactLabel}</span>
        <span className="hidden sm:inline">
          {baseWallet.isConnected && baseWallet.address
            ? displayBaseAddress(baseWallet.address)
            : "Connect Wallet"}
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
        aria-label={
          solanaWallet.isConnected && solanaWallet.address
            ? `Connected wallet ${truncateAddress(solanaWallet.address)}`
            : "Connect Wallet"
        }
        className="inline-flex h-10 max-w-[8.5rem] items-center border border-border bg-transparent px-3 text-[0.7rem] font-mono text-text-primary transition-colors cursor-pointer hover:bg-bg-hover sm:max-w-none sm:px-4"
      >
        <span className="truncate sm:hidden">
          {solanaWallet.isConnected && solanaWallet.address
            ? truncateAddress(solanaWallet.address)
            : "Connect"}
        </span>
        <span className="hidden sm:inline">
          {solanaWallet.isConnected && solanaWallet.address
            ? truncateAddress(solanaWallet.address)
            : "Connect Wallet"}
        </span>
      </button>
    );
  }

  return (
    <div className="flex items-center gap-1.5 sm:gap-2">
      {baseEnabled && (
        <button
          onClick={handleBaseClick}
          aria-label={
            baseWallet.isConnected && baseWallet.address
              ? `Connected Base wallet ${displayBaseAddress(baseWallet.address)}`
              : "Connect Base Wallet"
          }
          className={walletButtonClass}
        >
          {baseWallet.isConnected && baseWallet.address
            ? `Base ${displayBaseAddress(baseWallet.address)}`
            : "Connect Base Wallet"}
        </button>
      )}
      {solanaEnabled &&
        (solanaAvailable ? (
          <button
            onClick={handleSolanaClick}
            aria-label={
              solanaWallet.isConnected && solanaWallet.address
                ? `Connected Solana wallet ${truncateAddress(solanaWallet.address)}`
                : "Connect Solana Wallet"
            }
            className={walletButtonClass}
          >
            {solanaWallet.isConnected && solanaWallet.address
              ? `Sol ${truncateAddress(solanaWallet.address)}`
              : "Connect Solana Wallet"}
          </button>
        ) : (
          <a
            href="https://phantom.app/"
            target="_blank"
            rel="noreferrer"
            className={walletButtonClass}
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
    <header className="fixed inset-x-0 top-0 z-sticky border-b border-border bg-bg-base text-text-primary">
      <div className="flex min-w-0 items-center justify-between px-4 py-3 sm:px-8 sm:py-4">
        <Link href="/" className="flex shrink-0 items-center group">
          <BrandLogo compact />
        </Link>
        <nav className="hidden lg:flex items-center gap-6 ml-5">
          {primaryLinks.map(({ href, label }) => {
            const active =
              pathname === href ||
              (href !== "/" && pathname.startsWith(href));
            return (
              <Link
                key={href}
                href={href}
                className={cn(
                  "rounded-sm px-2 py-1 font-mono text-[0.85rem] uppercase leading-none transition-colors",
                  active
                    ? "bg-bg-secondary text-text-primary"
                    : "text-text-muted hover:text-text-primary",
                )}
              >
                {label}
              </Link>
            );
          })}
        </nav>

        <div className="hidden lg:flex min-w-0 flex-1 justify-center px-6">
          <form
            onSubmit={handleSearchSubmit}
            className="relative w-full max-w-[540px]"
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
                "w-full h-9 pl-9 pr-12 text-sm font-mono",
                "bg-transparent border border-border",
                "text-text-primary placeholder:text-text-muted",
                "focus:outline-none focus:border-border-hover",
                "transition-colors",
              )}
            />
          </form>
        </div>

        <div className="ml-auto flex shrink-0 items-center gap-2 sm:gap-3">
          {sessionRestored && hasSession ? <NotificationBell /> : null}
          <ConnectWalletButton />
          <SidebarMenu />
        </div>
      </div>
    </header>
  );
}
