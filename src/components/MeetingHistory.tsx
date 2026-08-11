import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { listAiProviders } from "../lib/ai/api";
import type { ProviderInfo } from "../lib/ai/types";
import {
  getMeeting,
  searchMeetings,
  type MeetingDetail,
  type MeetingListItem,
  type MeetingSearchFilters,
  type MeetingStatus,
} from "../lib/meetings";
import { MeetingDetailSheet } from "./MeetingDetailSheet";

const SEARCH_DEBOUNCE_MS = 280;
const WEEKDAY_LABELS = ["Lun", "Mar", "Mer", "Jeu", "Ven", "Sam", "Dim"];
const MAX_CALENDAR_PAGES = 40;

export interface HistoryNavigationState {
  query: string;
  dateFrom: string;
  dateTo: string;
  status: MeetingStatus | "";
  providerId: string;
  calendarMonth: { year: number; month: number };
}

export function defaultHistoryNavigationState(
  now: Date = new Date(),
): HistoryNavigationState {
  return {
    query: "",
    dateFrom: "",
    dateTo: "",
    status: "",
    providerId: "",
    calendarMonth: { year: now.getFullYear(), month: now.getMonth() },
  };
}

interface MeetingHistoryProps {
  initialSelectedId?: string | null;
  onSelectedIdChange?: (id: string | null) => void;
  navigationState?: HistoryNavigationState;
  onNavigationStateChange?: (state: HistoryNavigationState) => void;
}

function toDateKey(date: Date): string {
  const year = date.getFullYear();
  const month = (date.getMonth() + 1).toString().padStart(2, "0");
  const day = date.getDate().toString().padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function monthRange(year: number, month: number): { from: string; to: string } {
  const from = `${year}-${(month + 1).toString().padStart(2, "0")}-01`;
  const lastDay = new Date(year, month + 1, 0).getDate();
  const to = `${year}-${(month + 1).toString().padStart(2, "0")}-${lastDay.toString().padStart(2, "0")}`;
  return { from, to };
}

function meetingDateKey(meeting: MeetingListItem): string {
  const raw = meeting.startedAt ?? meeting.createdAt;
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) {
    return raw.slice(0, 10);
  }
  return toDateKey(date);
}

function actionBadge(meeting: MeetingListItem): string {
  switch (meeting.status) {
    case "completed":
      return "Terminée";
    case "processing":
      return "Traitement";
    case "recording":
      return "En cours";
    default:
      return "Brouillon";
  }
}

function buildSearchFilters(
  state: Pick<HistoryNavigationState, "query" | "dateFrom" | "dateTo" | "status" | "providerId">,
  cursor?: string,
): MeetingSearchFilters {
  const trimmedQuery = state.query.trim();
  return {
    ...(trimmedQuery ? { query: trimmedQuery } : {}),
    ...(state.status ? { status: state.status } : {}),
    ...(state.providerId ? { providerId: state.providerId } : {}),
    ...(state.dateFrom ? { dateFrom: state.dateFrom } : {}),
    ...(state.dateTo ? { dateTo: state.dateTo } : {}),
    ...(cursor ? { cursor } : {}),
  };
}

export function MeetingHistory({
  initialSelectedId = null,
  onSelectedIdChange,
  navigationState,
  onNavigationStateChange,
}: MeetingHistoryProps) {
  const [internalNav, setInternalNav] = useState<HistoryNavigationState>(
    () => navigationState ?? defaultHistoryNavigationState(),
  );
  const nav = navigationState ?? internalNav;

  const updateNav = useCallback(
    (patch: Partial<HistoryNavigationState>) => {
      const next = { ...nav, ...patch };
      if (onNavigationStateChange) {
        onNavigationStateChange(next);
      } else {
        setInternalNav(next);
      }
    },
    [nav, onNavigationStateChange],
  );

  const [debouncedQuery, setDebouncedQuery] = useState(nav.query.trim());
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [results, setResults] = useState<MeetingListItem[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [calendarMeetings, setCalendarMeetings] = useState<MeetingListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(initialSelectedId);
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const searchRequestId = useRef(0);
  const calendarRequestId = useRef(0);
  const detailRequestId = useRef(0);

  useEffect(() => {
    setSelectedId(initialSelectedId);
  }, [initialSelectedId]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDebouncedQuery(nav.query.trim());
    }, SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [nav.query]);

  useEffect(() => {
    void listAiProviders()
      .then(setProviders)
      .catch(() => undefined);
  }, []);

  const listFilters = useMemo(
    () =>
      buildSearchFilters({
        query: debouncedQuery,
        dateFrom: nav.dateFrom,
        dateTo: nav.dateTo,
        status: nav.status,
        providerId: nav.providerId,
      }),
    [debouncedQuery, nav.dateFrom, nav.dateTo, nav.status, nav.providerId],
  );

  const loadResults = useCallback(
    async (filters: MeetingSearchFilters, cursor?: string) => {
      const requestId = ++searchRequestId.current;
      const append = Boolean(cursor);
      if (append) {
        setLoadingMore(true);
      } else {
        setLoading(true);
        setNextCursor(null);
        setResults([]);
      }
      setError(null);
      try {
        const page = await searchMeetings({
          ...filters,
          ...(cursor ? { cursor } : {}),
        });
        if (requestId !== searchRequestId.current) {
          return;
        }
        setNextCursor(page.nextCursor);
        setResults((current) => (append ? appendUnique(current, page.items) : page.items));
      } catch (err) {
        if (requestId !== searchRequestId.current) {
          return;
        }
        setError(err instanceof Error ? err.message : "Chargement impossible.");
        if (!append) {
          setResults([]);
          setNextCursor(null);
        }
      } finally {
        if (requestId === searchRequestId.current) {
          setLoading(false);
          setLoadingMore(false);
        }
      }
    },
    [],
  );

  useEffect(() => {
    void loadResults(listFilters);
  }, [listFilters, loadResults]);

  useEffect(() => {
    const requestId = ++calendarRequestId.current;
    const range = monthRange(nav.calendarMonth.year, nav.calendarMonth.month);

    void (async () => {
      try {
        const collected: MeetingListItem[] = [];
        let cursor: string | undefined;
        for (let page = 0; page < MAX_CALENDAR_PAGES; page += 1) {
          const result = await searchMeetings({
            dateFrom: range.from,
            dateTo: range.to,
            ...(cursor ? { cursor } : {}),
          });
          if (requestId !== calendarRequestId.current) {
            return;
          }
          collected.push(...result.items);
          if (!result.nextCursor) {
            break;
          }
          cursor = result.nextCursor;
        }
        if (requestId === calendarRequestId.current) {
          setCalendarMeetings(appendUnique([], collected));
        }
      } catch {
        if (requestId === calendarRequestId.current) {
          setCalendarMeetings([]);
        }
      }
    })();
  }, [nav.calendarMonth.year, nav.calendarMonth.month]);

  useEffect(() => {
    if (!selectedId) {
      detailRequestId.current += 1;
      setDetail(null);
      setDetailLoading(false);
      return;
    }

    const requestId = ++detailRequestId.current;
    setDetail(null);
    setDetailLoading(true);
    void getMeeting(selectedId)
      .then((meeting) => {
        if (requestId === detailRequestId.current) {
          setDetail(meeting);
        }
      })
      .catch((err) => {
        if (requestId !== detailRequestId.current) {
          return;
        }
        setError(err instanceof Error ? err.message : "Chargement impossible.");
        setSelectedId(null);
        onSelectedIdChange?.(null);
      })
      .finally(() => {
        if (requestId === detailRequestId.current) {
          setDetailLoading(false);
        }
      });
  }, [selectedId, onSelectedIdChange]);

  const countsByDay = useMemo(() => {
    const counts = new Map<string, number>();
    for (const meeting of calendarMeetings) {
      const key = meetingDateKey(meeting);
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return counts;
  }, [calendarMeetings]);

  const calendarCells = useMemo(() => {
    const { year, month } = nav.calendarMonth;
    const firstDay = new Date(year, month, 1);
    const startOffset = (firstDay.getDay() + 6) % 7;
    const daysInMonth = new Date(year, month + 1, 0).getDate();
    const cells: Array<{ day: number | null; key: string | null }> = [];

    for (let index = 0; index < startOffset; index += 1) {
      cells.push({ day: null, key: null });
    }
    for (let day = 1; day <= daysInMonth; day += 1) {
      const key = `${year}-${(month + 1).toString().padStart(2, "0")}-${day.toString().padStart(2, "0")}`;
      cells.push({ day, key });
    }
    return cells;
  }, [nav.calendarMonth]);

  const monthLabel = new Date(nav.calendarMonth.year, nav.calendarMonth.month, 1).toLocaleDateString(
    "fr-FR",
    { month: "long", year: "numeric" },
  );

  const selectMeeting = useCallback(
    (id: string) => {
      setDetail(null);
      setSelectedId(id);
      onSelectedIdChange?.(id);
    },
    [onSelectedIdChange],
  );

  if (selectedId && detail) {
    return (
      <MeetingDetailSheet
        detail={detail}
        onBack={() => {
          setSelectedId(null);
          onSelectedIdChange?.(null);
        }}
        onDeleted={() => void loadResults(listFilters)}
      />
    );
  }

  const hasActiveFilters = Boolean(
    debouncedQuery || nav.dateFrom || nav.dateTo || nav.status || nav.providerId,
  );
  const showEmptyHistory = !loading && results.length === 0 && !hasActiveFilters;
  const showNoMatches = !loading && results.length === 0 && hasActiveFilters;

  return (
    <div className="meeting-history">
      <div className="lm-heading">
        <div>
          <p className="lm-kicker">Votre mémoire</p>
          <h2>
            Retrouvez une décision,
            <br />
            pas un fichier.
          </h2>
        </div>
      </div>

      <label className="meeting-history__search lm-search">
        <span aria-hidden="true">⌕</span>
        <span className="visually-hidden">Rechercher une réunion</span>
        <input
          type="search"
          value={nav.query}
          onChange={(event) => updateNav({ query: event.target.value })}
          placeholder="Rechercher dans toutes les réunions"
          autoComplete="off"
          spellCheck={false}
        />
      </label>

      <div className="meeting-history__filters">
        <div className="meeting-history__filter-row">
          <label>
            Du
            <input
              type="date"
              value={nav.dateFrom}
              onChange={(event) => updateNav({ dateFrom: event.target.value })}
            />
          </label>
          <label>
            Au
            <input
              type="date"
              value={nav.dateTo}
              onChange={(event) => updateNav({ dateTo: event.target.value })}
            />
          </label>
          <label>
            Statut
            <select
              value={nav.status}
              onChange={(event) =>
                updateNav({ status: event.target.value as MeetingStatus | "" })
              }
            >
              <option value="">Tous</option>
              <option value="draft">Brouillon</option>
              <option value="recording">Enregistrement</option>
              <option value="processing">Traitement</option>
              <option value="completed">Terminée</option>
            </select>
          </label>
          <label>
            Fournisseur IA
            <select
              value={nav.providerId}
              onChange={(event) => updateNav({ providerId: event.target.value })}
            >
              <option value="">Tous</option>
              {providers.map((provider) => (
                <option key={provider.id} value={provider.id}>
                  {provider.displayName}
                </option>
              ))}
            </select>
          </label>
        </div>

        {(nav.dateFrom || nav.dateTo) && (
          <button
            type="button"
            className="meeting-history__clear-dates"
            onClick={() => updateNav({ dateFrom: "", dateTo: "" })}
          >
            Effacer le filtre de dates
          </button>
        )}
      </div>

      <section className="lm-panel meeting-history__calendar">
        <div className="meeting-history__calendar-header">
          <button
            type="button"
            aria-label="Mois précédent"
            onClick={() => {
              const date = new Date(nav.calendarMonth.year, nav.calendarMonth.month - 1, 1);
              updateNav({
                calendarMonth: { year: date.getFullYear(), month: date.getMonth() },
              });
            }}
          >
            ‹
          </button>
          <p className="meeting-history__calendar-title">{monthLabel}</p>
          <button
            type="button"
            aria-label="Mois suivant"
            onClick={() => {
              const date = new Date(nav.calendarMonth.year, nav.calendarMonth.month + 1, 1);
              updateNav({
                calendarMonth: { year: date.getFullYear(), month: date.getMonth() },
              });
            }}
          >
            ›
          </button>
        </div>

        <div
          className="meeting-history__calendar-grid"
          role="grid"
          aria-label="Calendrier des réunions"
        >
          {WEEKDAY_LABELS.map((label) => (
            <div key={label} className="meeting-history__calendar-weekday" role="columnheader">
              {label}
            </div>
          ))}
          {calendarCells.map((cell, index) => {
            if (cell.day === null || cell.key === null) {
              return (
                <div
                  key={`empty-${index}`}
                  className="meeting-history__calendar-cell meeting-history__calendar-cell--empty"
                />
              );
            }

            const count = countsByDay.get(cell.key) ?? 0;
            const isSelected = nav.dateFrom === cell.key && nav.dateTo === cell.key;

            return (
              <button
                key={cell.key}
                type="button"
                className={`meeting-history__calendar-cell${count > 0 ? " meeting-history__calendar-cell--has-meetings" : ""}${isSelected ? " meeting-history__calendar-cell--selected" : ""}`}
                onClick={() => updateNav({ dateFrom: cell.key!, dateTo: cell.key! })}
                aria-label={`${cell.day} ${monthLabel}${count > 0 ? `, ${count} réunion(s)` : ""}`}
              >
                <span>{cell.day}</span>
                {count > 0 && <span className="meeting-history__calendar-dot" aria-hidden="true" />}
              </button>
            );
          })}
        </div>
      </section>

      {detailLoading && <p className="progress-message">Chargement du détail…</p>}
      {loading && <p className="progress-message">Chargement…</p>}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}

      {showEmptyHistory && (
        <div className="lm-panel lm-empty">
          <div className="lm-empty-inner">
            <h3>Aucune réunion</h3>
            <p className="lm-subtle">Importez ou enregistrez un audio depuis Aujourd’hui.</p>
          </div>
        </div>
      )}

      {showNoMatches && (
        <div className="lm-panel lm-empty">
          <div className="lm-empty-inner">
            <h3>Aucun résultat</h3>
            <p className="lm-subtle">
              {debouncedQuery
                ? `Aucune réunion ne correspond à « ${debouncedQuery} ».`
                : "Aucune réunion ne correspond aux filtres sélectionnés."}
            </p>
          </div>
        </div>
      )}

      <div className="history-rows">
        {results.map((meeting) => (
          <button
            key={meeting.id}
            type="button"
            className="history-row"
            onClick={() => selectMeeting(meeting.id)}
          >
            <i aria-hidden="true" />
            <div>
              <h3>{meeting.title}</h3>
              <span>
                {meeting.snippet?.trim() ||
                  "Ouvrir la fiche pour consulter l’essentiel de cette réunion."}
              </span>
            </div>
            <em>{actionBadge(meeting)}</em>
          </button>
        ))}
      </div>
      {nextCursor && (
        <div className="meeting-history__more">
          <button
            type="button"
            onClick={() => void loadResults(listFilters, nextCursor)}
            disabled={loadingMore}
          >
            {loadingMore ? "Chargement…" : "Charger plus de réunions"}
          </button>
        </div>
      )}
    </div>
  );
}

function appendUnique(current: MeetingListItem[], incoming: MeetingListItem[]): MeetingListItem[] {
  const seen = new Set(current.map((meeting) => meeting.id));
  const additions = incoming.filter((meeting) => !seen.has(meeting.id));
  return [...current, ...additions];
}
