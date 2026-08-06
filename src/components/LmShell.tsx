import type { ReactNode } from "react";

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
  return (
    <span className="lm-mark" aria-hidden="true">
      <svg viewBox="0 0 24 24">
        <circle cx="12" cy="13" r="8.2" fill="none" stroke="currentColor" strokeWidth="2.1" />
        <path d="M12 13l3.8-6.3" fill="none" stroke="currentColor" strokeWidth="2.1" />
        <circle cx="12" cy="13" r="1.2" fill="currentColor" />
        <path className="lm-clock-red" d="M11.3 2h1.4v2.1h-1.4z" />
      </svg>
    </span>
  );
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
