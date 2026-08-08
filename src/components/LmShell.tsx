import type { ReactNode } from "react";

import { formatDuration } from "../lib/audio";
import { ClockMark } from "./ClockMark";

export type AppScreen = "meeting" | "history" | "settings";

interface LmShellProps {
  active: AppScreen;
  onNavigate: (screen: AppScreen) => void;
  children: ReactNode;
  isRecording?: boolean;
  recordingDurationSecs?: number | null;
}

const NAV_ITEMS: Array<{ id: AppScreen; label: string }> = [
  { id: "meeting", label: "Réunion courante" },
  { id: "history", label: "Historique" },
  { id: "settings", label: "Réglages" },
];

function BrandMark() {
  return <ClockMark />;
}

export function LmShell({
  active,
  onNavigate,
  children,
  isRecording = false,
  recordingDurationSecs = null,
}: LmShellProps) {
  return (
    <div className="lm-shell">
      <div className="lm-ambient" aria-hidden="true">
        <span className="lm-ambient__orb lm-ambient__orb--a" />
        <span className="lm-ambient__orb lm-ambient__orb--b" />
        <span className="lm-ambient__orb lm-ambient__orb--c" />
      </div>
      <aside className="lm-nav" aria-label="Navigation">
        <div className="lm-nav-brand">
          <BrandMark />
          La Minute
        </div>
        {isRecording ? (
          <button
            type="button"
            className="lm-mic-live"
            onClick={() => onNavigate("meeting")}
            aria-label={`Micro actif — retour à la réunion courante${
              recordingDurationSecs != null
                ? `, durée ${formatDuration(recordingDurationSecs)}`
                : ""
            }`}
          >
            <span className="lm-mic-live__dot" aria-hidden="true" />
            <span className="lm-mic-live__label">Micro actif</span>
            {recordingDurationSecs != null ? (
              <span className="lm-mic-live__chrono" aria-hidden="true">
                {formatDuration(recordingDurationSecs)}
              </span>
            ) : null}
          </button>
        ) : null}
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
