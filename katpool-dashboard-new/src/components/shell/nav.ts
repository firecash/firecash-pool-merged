import {
  Activity,
  Blocks,
  LayoutDashboard,
  Pickaxe,
  Trophy,
  Wallet,
  type LucideIcon,
} from "lucide-react";

export interface NavItem {
  href: string;
  label: string;
  /** Condensed label for the mobile bottom bar (falls back to first word). */
  shortLabel?: string;
  icon: LucideIcon;
  /** Match exactly (true) or by prefix (false). */
  exact?: boolean;
  /** Render as a highlighted call-to-action (top of the stack). */
  cta?: boolean;
}

export const NAV_ITEMS: NavItem[] = [
  { href: "/start", label: "Start Mining", shortLabel: "Start", icon: Pickaxe, cta: true },
  { href: "/", label: "Overview", icon: LayoutDashboard, exact: true },
  { href: "/blocks", label: "Blocks", icon: Blocks },
  { href: "/payouts", label: "Payouts", icon: Wallet },
  { href: "/leaders", label: "Leaderboard", shortLabel: "Leaders", icon: Trophy },
  { href: "/status", label: "Status", icon: Activity },
];
