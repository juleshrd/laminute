import { useCallback, useEffect, useRef, useState } from "react";

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
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const searchRequestId = useRef(0);
  const detailRequestId = useRef(0);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDebouncedQuery(query.trim());
    }, SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [query]);

  const loadResults = useCallback(async (searchQuery: string, cursor?: string) => {
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
        ...(searchQuery ? { query: searchQuery } : {}),
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
  }, []);

  useEffect(() => {
    void loadResults(debouncedQuery);
  }, [debouncedQuery, loadResults]);

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
      })
      .finally(() => {
        if (requestId === detailRequestId.current) {
          setDetailLoading(false);
        }
      });
  }, [selectedId]);

  const selectMeeting = useCallback((id: string) => {
    setDetail(null);
    setSelectedId(id);
  }, []);

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
              onClick={() => selectMeeting(meeting.id)}
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
      {nextCursor && (
        <div className="meeting-history__more">
          <button
            type="button"
            onClick={() => void loadResults(debouncedQuery, nextCursor)}
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
