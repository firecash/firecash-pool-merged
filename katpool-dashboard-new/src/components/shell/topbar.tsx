import { Brand } from "./brand";
import { MobileNav } from "./mobile-nav";
import { ThemeToggle } from "./theme-toggle";
import { WalletSearch } from "./wallet-search";
import { PriceTicker } from "./price-ticker";

/** Sticky top command bar: menu, search, price ticker, theme. */
export function Topbar() {
  return (
    <header className="glass sticky top-0 z-40 flex h-16 items-center gap-3 border-b border-border px-4 lg:px-6">
      <MobileNav />
      <Brand className="lg:hidden" />
      <div className="hidden flex-1 md:block">
        <WalletSearch />
      </div>
      <div className="flex flex-1 items-center justify-end gap-2 md:flex-none">
        <PriceTicker />
        <ThemeToggle />
      </div>
    </header>
  );
}
