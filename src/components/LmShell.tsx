import type { ReactNode } from "react";

import { ClockMark } from "./ClockMark";

export type AppScreen = "meeting" | "history" | "settings";

interface LmShellProps {
  active: AppScreen;
  onNavigate: (screen: AppScreen) => void;
  children: ReactNode;
}

const NAV_ITEMS: Array<{ id: AppScreen; label: string }> = [
  { id: "meeting", label: "Réunion courante" },
  { id: "history", label: "Historique" },
  { id: "settings", label: "Réglages" },
];

function BrandMark() {
  return <ClockMark />;
}

export function LmShell({ active, onNavigate, children }: LmShellProps) {
  return (
    <div className="lm-shell">
      <aside className="lm-nav" aria-label="Navigation">
        <div className="lm-nav-brand">
          <BrandMark />
          La Minute
        </div>
        {NAV_ITEMS.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`lm-navitem${active === item.id ? " is-current" : ""}`}
            aria-current={active === item.id ? "page" : undefined}
            onClick={() => onNavigate(item.id)}
          >
            {item.label}
          </button>
        ))}
      </aside>
      <div className="lm-main">{children}</div>
    </div>
  );
}

export { BrandMark };
