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
  { href: "/agents", label: "Swarm" },
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
      className="h-9 px-4 text-[0.75rem] font-mono inline-flex items-center border border-border text-text-primary bg-transparent hover:bg-bg-hover transition-colors"
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
        className="h-9 max-w-[8.5rem] border border-border bg-transparent px-3 text-[0.75rem] font-mono text-text-primary transition-colors cursor-pointer sm:max-w-none sm:px-4 hover:bg-bg-hover"
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
        className="h-9 max-w-[8.5rem] border border-border bg-transparent px-3 text-[0.75rem] font-mono text-text-primary transition-colors cursor-pointer sm:max-w-none sm:px-4 hover:bg-bg-hover"
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
          className="h-9 border border-border bg-transparent px-3 text-[0.75rem] font-mono text-text-primary transition-colors hover:bg-bg-hover"
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
            className="h-9 border border-border bg-transparent px-3 text-[0.75rem] font-mono text-text-primary transition-colors hover:bg-bg-hover"
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
            className="h-9 px-3 text-[0.75rem] font-mono border border-border inline-flex items-center text-text-primary bg-transparent hover:bg-bg-hover transition-colors"
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
                  "font-mono text-[0.85rem] uppercase leading-none transition-colors",
                  active
                    ? "text-text-primary"
                    : "text-text-muted hover:text-text-primary",
                )}
              >
                {label}
              </Link>
            );
          })}
        </nav>

        <div className="hidden lg:flex min-w-0 flex-1 justify-center px-8">
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
