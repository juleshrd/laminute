import { useCallback, useEffect, useState } from "react";

import {
  getMeeting,
  listMeetings,
  meetingDisplayDate,
  meetingStatusLabel,
  type MeetingDetail,
  type MeetingSummary,
} from "../lib/meetings";
import { MeetingDetailSheet } from "./MeetingDetailSheet";

function shortDateParts(meeting: MeetingSummary): { day: string; month: string } {
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
  const [results, setResults] = useState<MeetingSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  const loadResults = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const items = await listMeetings();
      setResults(items);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Chargement impossible.");
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadResults();
  }, [loadResults]);

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
        onDeleted={() => void loadResults()}
      />
    );
  }

  return (
    <div className="meeting-history">
      <div className="lm-heading">
        <div>
          <h2>Réunions</h2>
          <p className="lm-subtle">Historique local des réunions traitées.</p>
        </div>
      </div>

      {detailLoading && <p className="progress-message">Chargement du détail…</p>}
      {loading && <p className="progress-message">Chargement…</p>}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}

      {!loading && results.length === 0 && (
        <div className="lm-panel lm-empty">
          <div className="lm-empty-inner">
            <h3>Aucune réunion</h3>
            <p className="lm-subtle">Importez ou enregistrez un audio depuis Réunion courante.</p>
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
              </div>
              <span className="lm-status">{meetingStatusLabel(meeting.status)}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
