import { useCallback, useEffect, useMemo, useState } from "react";

import { listAiProviders } from "../lib/ai/api";
import type { ProviderInfo } from "../lib/ai/types";
import {
  getMeeting,
  meetingDisplayDate,
  meetingStatusLabel,
  searchMeetings,
  type MeetingDetail,
  type MeetingListItem,
  type MeetingSearchFilters,
  type MeetingStatus,
} from "../lib/meetings";
import { MeetingDetailSheet } from "./MeetingDetailSheet";

const WEEKDAY_LABELS = ["Lun", "Mar", "Mer", "Jeu", "Ven", "Sam", "Dim"];

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

export function MeetingHistory() {
  const today = new Date();
  const [calendarMonth, setCalendarMonth] = useState({
    year: today.getFullYear(),
    month: today.getMonth(),
  });
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [status, setStatus] = useState<MeetingStatus | "">("");
  const [providerId, setProviderId] = useState("");
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [results, setResults] = useState<MeetingListItem[]>([]);
  const [calendarMeetings, setCalendarMeetings] = useState<MeetingListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedQuery(query.trim()), 300);
    return () => window.clearTimeout(timer);
  }, [query]);

  useEffect(() => {
    void listAiProviders()
      .then(setProviders)
      .catch(() => undefined);
  }, []);

  const filters = useMemo<MeetingSearchFilters>(
    () => ({
      query: debouncedQuery || undefined,
      status: status || undefined,
      providerId: providerId || undefined,
      dateFrom: dateFrom || undefined,
      dateTo: dateTo || undefined,
    }),
    [debouncedQuery, status, providerId, dateFrom, dateTo],
  );

  const loadResults = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const items = await searchMeetings(filters);
      setResults(items);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Recherche impossible.");
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, [filters]);

  useEffect(() => {
    void loadResults();
  }, [loadResults]);

  useEffect(() => {
    const range = monthRange(calendarMonth.year, calendarMonth.month);
    void searchMeetings({ dateFrom: range.from, dateTo: range.to })
      .then(setCalendarMeetings)
      .catch(() => setCalendarMeetings([]));
  }, [calendarMonth]);

  useEffect(() => {
    if (!selectedId) {
      setDetail(null);
      return;
    }

    setDetailLoading(true);
    void getMeeting(selectedId)
      .then(setDetail)
      .catch((err) => {
        setError(err instanceof Error ? err.message : "Chargement impossible.");
        setSelectedId(null);
      })
      .finally(() => setDetailLoading(false));
  }, [selectedId]);

  const countsByDay = useMemo(() => {
    const counts = new Map<string, number>();
    for (const meeting of calendarMeetings) {
      const key = meetingDateKey(meeting);
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return counts;
  }, [calendarMeetings]);

  const calendarCells = useMemo(() => {
    const { year, month } = calendarMonth;
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
  }, [calendarMonth]);

  const monthLabel = new Date(calendarMonth.year, calendarMonth.month, 1).toLocaleDateString(
    "fr-FR",
    { month: "long", year: "numeric" },
  );

  function selectDay(key: string) {
    setDateFrom(key);
    setDateTo(key);
  }

  function clearDayFilter() {
    setDateFrom("");
    setDateTo("");
  }

  if (selectedId && detail) {
    return <MeetingDetailSheet detail={detail} onBack={() => setSelectedId(null)} />;
  }

  return (
    <div className="meeting-history">
      <section className="panel">
        <h2>Rechercher</h2>
        <div className="meeting-history__filters">
          <input
            type="search"
            placeholder="Rechercher par titre, transcription ou compte-rendu…"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            aria-label="Recherche textuelle"
          />

          <div className="meeting-history__filter-row">
            <label>
              Du
              <input
                type="date"
                value={dateFrom}
                onChange={(event) => setDateFrom(event.target.value)}
              />
            </label>
            <label>
              Au
              <input
                type="date"
                value={dateTo}
                onChange={(event) => setDateTo(event.target.value)}
              />
            </label>
            <label>
              Statut
              <select value={status} onChange={(event) => setStatus(event.target.value as MeetingStatus | "")}>
                <option value="">Tous</option>
                <option value="draft">Brouillon</option>
                <option value="recording">Enregistrement</option>
                <option value="processing">Traitement</option>
                <option value="completed">Terminée</option>
              </select>
            </label>
            <label>
              Fournisseur IA
              <select value={providerId} onChange={(event) => setProviderId(event.target.value)}>
                <option value="">Tous</option>
                {providers.map((provider) => (
                  <option key={provider.id} value={provider.id}>
                    {provider.displayName}
                  </option>
                ))}
              </select>
            </label>
          </div>

          {(dateFrom || dateTo) && (
            <button type="button" className="meeting-history__clear-dates" onClick={clearDayFilter}>
              Effacer le filtre de dates
            </button>
          )}
        </div>
      </section>

      <section className="panel meeting-history__calendar">
        <div className="meeting-history__calendar-header">
          <button
            type="button"
            aria-label="Mois précédent"
            onClick={() =>
              setCalendarMonth((current) => {
                const date = new Date(current.year, current.month - 1, 1);
                return { year: date.getFullYear(), month: date.getMonth() };
              })
            }
          >
            ‹
          </button>
          <h2>{monthLabel}</h2>
          <button
            type="button"
            aria-label="Mois suivant"
            onClick={() =>
              setCalendarMonth((current) => {
                const date = new Date(current.year, current.month + 1, 1);
                return { year: date.getFullYear(), month: date.getMonth() };
              })
            }
          >
            ›
          </button>
        </div>

        <div className="meeting-history__calendar-grid" role="grid" aria-label="Calendrier des réunions">
          {WEEKDAY_LABELS.map((label) => (
            <div key={label} className="meeting-history__calendar-weekday" role="columnheader">
              {label}
            </div>
          ))}
          {calendarCells.map((cell, index) => {
            if (cell.day === null || cell.key === null) {
              return <div key={`empty-${index}`} className="meeting-history__calendar-cell meeting-history__calendar-cell--empty" />;
            }

            const count = countsByDay.get(cell.key) ?? 0;
            const isSelected = dateFrom === cell.key && dateTo === cell.key;

            return (
              <button
                key={cell.key}
                type="button"
                className={`meeting-history__calendar-cell${count > 0 ? " meeting-history__calendar-cell--has-meetings" : ""}${isSelected ? " meeting-history__calendar-cell--selected" : ""}`}
                onClick={() => selectDay(cell.key!)}
                aria-label={`${cell.day} ${monthLabel}${count > 0 ? `, ${count} réunion(s)` : ""}`}
              >
                <span>{cell.day}</span>
                {count > 0 && <span className="meeting-history__calendar-dot" aria-hidden="true" />}
              </button>
            );
          })}
        </div>
      </section>

      <section className="panel">
        <h2>Résultats {loading ? "" : `(${results.length})`}</h2>

        {detailLoading && <p className="progress-message">Chargement du détail…</p>}
        {loading && <p className="progress-message">Recherche en cours…</p>}
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}

        {!loading && results.length === 0 && (
          <p className="meeting-history__empty">Aucune réunion trouvée.</p>
        )}

        <ul className="meeting-history__list">
          {results.map((meeting) => (
            <li key={meeting.id}>
              <button
                type="button"
                className="meeting-history__item"
                onClick={() => setSelectedId(meeting.id)}
              >
                <span className="meeting-history__item-title">{meeting.title}</span>
                <span className="meeting-history__item-meta">
                  {meetingDisplayDate(meeting)} · {meetingStatusLabel(meeting.status)}
                </span>
                {meeting.snippet && (
                  <span className="meeting-history__item-snippet">{meeting.snippet}</span>
                )}
              </button>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
