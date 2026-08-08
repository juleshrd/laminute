import { useCallback, useEffect, useRef, useState } from "react";

import {
  getMeeting,
  searchMeetings,
  type MeetingDetail,
  type MeetingListItem,
} from "../lib/meetings";
import { MeetingDetailSheet } from "./MeetingDetailSheet";

const SEARCH_DEBOUNCE_MS = 280;

interface MeetingHistoryProps {
  initialSelectedId?: string | null;
  onSelectedIdChange?: (id: string | null) => void;
}

function actionBadge(meeting: MeetingListItem): string {
  return meetingStatusLabelShort(meeting);
}

function meetingStatusLabelShort(meeting: MeetingListItem): string {
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

export function MeetingHistory({
  initialSelectedId = null,
  onSelectedIdChange,
}: MeetingHistoryProps) {
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [results, setResults] = useState<MeetingListItem[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(initialSelectedId);
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const searchRequestId = useRef(0);
  const detailRequestId = useRef(0);

  useEffect(() => {
    setSelectedId(initialSelectedId);
  }, [initialSelectedId]);

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
        onSelectedIdChange?.(null);
      })
      .finally(() => {
        if (requestId === detailRequestId.current) {
          setDetailLoading(false);
        }
      });
  }, [selectedId, onSelectedIdChange]);

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
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Rechercher dans toutes les réunions"
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
            <p className="lm-subtle">Importez ou enregistrez un audio depuis Aujourd’hui.</p>
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
