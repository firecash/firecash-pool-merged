import { type ReactNode } from "react";
import { Sidebar } from "./sidebar";
import { Topbar } from "./topbar";
import { BottomNav } from "./bottom-nav";
import { WalletSearch } from "./wallet-search";

/** The persistent application chrome: sidebar, top bar, and content slot. */
export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="app-aurora min-h-screen overflow-x-clip">
      <div className="flex w-full">
        <Sidebar />
        <div className="flex min-w-0 flex-1 flex-col">
          <Topbar />
          <div className="mx-auto w-full max-w-[1600px] px-4 pt-4 md:hidden">
            <WalletSearch className="max-w-none" />
          </div>
          <main className="mx-auto w-full max-w-[1600px] flex-1 px-4 pb-24 pt-4 lg:px-6 lg:pb-10">
            {children}
          </main>
        </div>
      </div>
      <BottomNav />
    </div>
  );
}
