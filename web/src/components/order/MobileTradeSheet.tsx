"use client";

import { useState } from "react";
import {
  Sheet,
  SheetContent,
  SheetTitle,
} from "@/components/ui/sheet";
import { OrderForm } from "./OrderForm";
import { ExternalOrderForm } from "./ExternalOrderForm";
import type { Market } from "@/types";

interface MobileTradeSheetProps {
  market: Market;
}

export function MobileTradeSheet({ market }: MobileTradeSheetProps) {
  const [open, setOpen] = useState(false);

  const yesPrice = Math.round(market.yesPrice * 100);
  const noPrice = Math.round(market.noPrice * 100);

  return (
    <>
      <div className="fixed inset-x-0 bottom-0 z-sticky md:hidden border-t border-border bg-bg-base pb-[env(safe-area-inset-bottom)]">
        <div className="flex gap-px">
          <button
            type="button"
            onClick={() => setOpen(true)}
            className="flex-1 flex items-center justify-center gap-2 h-14 bg-green-600/15 text-green-400 font-mono text-sm uppercase tracking-wider transition-colors active:bg-green-600/25"
          >
            Yes
            <span className="text-xs opacity-70">{yesPrice}¢</span>
          </button>
          <button
            type="button"
            onClick={() => setOpen(true)}
            className="flex-1 flex items-center justify-center gap-2 h-14 bg-red-600/15 text-red-400 font-mono text-sm uppercase tracking-wider transition-colors active:bg-red-600/25"
          >
            No
            <span className="text-xs opacity-70">{noPrice}¢</span>
          </button>
        </div>
      </div>

      <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent
          side="bottom"
          className="max-h-[85vh] overflow-y-auto bg-bg-base border-border p-0 pb-[env(safe-area-inset-bottom)]"
        >
          <SheetTitle className="sr-only">Place order</SheetTitle>
          <div className="w-12 h-1 rounded-full bg-border mx-auto mt-3 mb-1" />
          <div className="px-4 pb-6">
            {market.isExternal ? (
              <ExternalOrderForm
                market={market}
                onSuccess={() => setOpen(false)}
              />
            ) : (
              <OrderForm
                market={market}
                onSuccess={() => setOpen(false)}
              />
            )}
          </div>
        </SheetContent>
      </Sheet>
    </>
  );
}
