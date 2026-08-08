import { useEffect, useState, type ReactNode } from "react";

import { formatDuration } from "../lib/audio";
import { formatDurationMs, searchMeetings, type MeetingListItem } from "../lib/meetings";
import { ClockMark } from "./ClockMark";
import { APP_NAME } from "../lib/app";

function listItemDurationMs(meeting: MeetingListItem): number | null {
  if (meeting.startedAt && meeting.endedAt) {
    const start = new Date(meeting.startedAt).getTime();
    const end = new Date(meeting.endedAt).getTime();
    if (!Number.isNaN(start) && !Number.isNaN(end) && end >= start) {
      return end - start;
    }
  }
  return null;
}

export type AppScreen = "meeting" | "history" | "settings";

interface LmShellProps {
  active: AppScreen;
  onNavigate: (screen: AppScreen) => void;
  onOpenMeeting?: (meetingId: string) => void;
  children: ReactNode;
  isRecording?: boolean;
  recordingDurationSecs?: number | null;
  onStopRecording?: () => void;
}

const PRIMARY_NAV: Array<{ id: Exclude<AppScreen, "settings">; label: string; icon: string }> = [
  { id: "meeting", label: "Aujourd’hui", icon: "◉" },
  { id: "history", label: "Historique", icon: "⌕" },
];

function BrandMark() {
  return <ClockMark className="lm-mark lm-mark--badge" />;
}

function relativeMeetingLabel(meeting: MeetingListItem): string {
  const raw = meeting.startedAt ?? meeting.createdAt;
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) {
    return "Récente";
  }

  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const startOfThatDay = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const dayDiff = Math.round(
    (startOfToday.getTime() - startOfThatDay.getTime()) / (24 * 60 * 60 * 1000),
  );

  let dayLabel = date.toLocaleDateString("fr-FR", { day: "numeric", month: "short" });
  if (dayDiff === 0) {
    dayLabel = "Aujourd’hui";
  } else if (dayDiff === 1) {
    dayLabel = "Hier";
  }

  const duration = listItemDurationMs(meeting);
  if (duration == null || duration <= 0) {
    return dayLabel;
  }
  return `${dayLabel} · ${formatDurationMs(duration)}`;
}

export function LmShell({
  active,
  onNavigate,
  onOpenMeeting,
  children,
  isRecording = false,
  recordingDurationSecs = null,
  onStopRecording,
}: LmShellProps) {
  const [recent, setRecent] = useState<MeetingListItem[]>([]);

  useEffect(() => {
    let cancelled = false;
    void searchMeetings({})
      .then((page) => {
        if (!cancelled) {
          setRecent(page.items.slice(0, 4));
        }
      })
      .catch(() => {
        if (!cancelled) {
          setRecent([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [active, isRecording]);

  return (
    <div className="lm-shell">
      <header className="lm-chrome">
        <span className="lm-chrome__title">{APP_NAME}</span>
        <div className="lm-chrome__mic">
          <span className={`lm-micro${isRecording ? " is-live" : ""}`}>
            {isRecording ? "● Micro actif" : "Micro prêt"}
            {isRecording && recordingDurationSecs != null
              ? ` · ${formatDuration(recordingDurationSecs)}`
              : ""}
          </span>
          {isRecording && onStopRecording ? (
            <button
              type="button"
              className="lm-chrome__stop"
              onClick={() => void onStopRecording()}
            >
              Arrêter
            </button>
          ) : null}
        </div>
      </header>

      <div className="lm-shell__body">
        <aside className="lm-nav" aria-label="Navigation">
          <div className="lm-nav-brand">
            <BrandMark />
            <strong>{APP_NAME}</strong>
          </div>

          <nav className="lm-nav__primary">
            {PRIMARY_NAV.map((item) => (
              <button
                key={item.id}
                type="button"
                className={`lm-navitem${active === item.id ? " is-current" : ""}`}
                aria-current={active === item.id ? "page" : undefined}
                onClick={() => onNavigate(item.id)}
              >
                <span className="lm-navitem__icon" aria-hidden="true">
                  {item.icon}
                </span>
                {item.label}
              </button>
            ))}
          </nav>

          {recent.length > 0 ? (
            <div className="lm-recent">
              <p className="lm-recent__label">Récentes</p>
              <ul className="lm-recent__list">
                {recent.map((meeting) => (
                  <li key={meeting.id}>
                    <button
                      type="button"
                      className="lm-recent__item"
                      onClick={() => {
                        if (onOpenMeeting) {
                          onOpenMeeting(meeting.id);
                        } else {
                          onNavigate("history");
                        }
                      }}
                    >
                      <b>{meeting.title}</b>
                      <span>{relativeMeetingLabel(meeting)}</span>
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}

          {isRecording ? (
            <button
              type="button"
              className="lm-mic-live"
              onClick={() => onNavigate("meeting")}
              aria-label={`Micro actif — retour à Aujourd’hui${
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

          <button
            type="button"
            className={`lm-navitem lm-navitem--settings${active === "settings" ? " is-current" : ""}`}
            aria-current={active === "settings" ? "page" : undefined}
            onClick={() => onNavigate("settings")}
          >
            <span className="lm-navitem__icon" aria-hidden="true">
              ⚙︎
            </span>
            Réglages
          </button>
        </aside>

        <div className="lm-main">{children}</div>
      </div>
    </div>
  );
}

export { BrandMark };
