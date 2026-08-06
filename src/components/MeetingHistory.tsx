import { useCallback, useEffect, useState } from "react";

import {
  getMeeting,
  meetingDisplayDate,
  meetingStatusLabel,
  searchMeetings,
  type MeetingDetail,
  type MeetingListItem,
} from "../lib/meetings";
import { MeetingDetailSheet } from "./MeetingDetailSheet";

const SEARCH_DEBOUNCE_MS = 280;

function shortDateParts(meeting: MeetingListItem): { day: string; month: string } {
  const raw = meeting.startedAt ?? meeting.createdAt;
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) {
    return { day: "—", month: "" };
  }
  return {
    day: date.toLocaleDateString("fr-FR", { day: "2-digit" }),
    month: date.toLocaleDateString("fr-FR", { month: "short" }).replace(".", ""),
  };
}

export function MeetingHistory() {
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [results, setResults] = useState<MeetingListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDebouncedQuery(query.trim());
    }, SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [query]);

  const loadResults = useCallback(async (searchQuery: string) => {
    setLoading(true);
    setError(null);
    try {
      const items = await searchMeetings(searchQuery ? { query: searchQuery } : {});
      setResults(items);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Chargement impossible.");
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadResults(debouncedQuery);
  }, [debouncedQuery, loadResults]);

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

  if (selectedId && detail) {
    return (
      <MeetingDetailSheet
        detail={detail}
        onBack={() => setSelectedId(null)}
        onDeleted={() => void loadResults(debouncedQuery)}
      />
    );
  }

  const trimmedQuery = debouncedQuery;
  const showEmptyHistory = !loading && results.length === 0 && !trimmedQuery;
  const showNoMatches = !loading && results.length === 0 && Boolean(trimmedQuery);

  return (
    <div className="meeting-history">
      <div className="lm-heading">
        <div>
          <h2>Réunions</h2>
          <p className="lm-subtle">Historique local des réunions traitées.</p>
        </div>
      </div>

      <label className="meeting-history__search">
        <span className="visually-hidden">Rechercher une réunion</span>
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Rechercher une réunion…"
          autoComplete="off"
          spellCheck={false}
        />
      </label>

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
            <p className="lm-subtle">Importez ou enregistrez un audio depuis Réunion courante.</p>
          </div>
        </div>
      )}

      {showNoMatches && (
        <div className="lm-panel lm-empty">
          <div className="lm-empty-inner">
            <h3>Aucun résultat</h3>
            <p className="lm-subtle">Aucune réunion ne correspond à « {trimmedQuery} ».</p>
          </div>
        </div>
      )}

      <div className="lm-list">
        {results.map((meeting) => {
          const date = shortDateParts(meeting);
          return (
            <button
              key={meeting.id}
              type="button"
              className="lm-listrow"
              onClick={() => setSelectedId(meeting.id)}
            >
              <div className="lm-date">
                {date.day}
                <br />
                {date.month}
              </div>
              <div>
                <h3>{meeting.title}</h3>
                <p className="lm-meta">{meetingDisplayDate(meeting)}</p>
                {meeting.snippet ? (
                  <p className="meeting-history__snippet">{meeting.snippet}</p>
                ) : null}
              </div>
              <span className="lm-status">{meetingStatusLabel(meeting.status)}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
