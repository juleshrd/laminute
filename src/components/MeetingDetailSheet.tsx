import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

import {
  deleteMeeting,
  getSummary,
  getTranscription,
  parseStoredSummary,
} from "../lib/meetings";
import {
  buildExportFilename,
  exportMeeting,
  exportMeetingPdf,
  writeExportBytes,
  writeExportFile,
} from "../lib/privacy";
import { buildReportMarkdown, reportExportMeta } from "../lib/reportExport";
import {
  formatDurationMs,
  meetingDisplayDate,
  meetingDurationMs,
  meetingStatusLabel,
  type MeetingDetail,
} from "../lib/meetings";
import type { SummaryRecord } from "../lib/ai/types";
import type { Transcription } from "../lib/transcription";
import { StructuredSummaryView } from "./StructuredSummaryView";
import "./StructuredSummaryPanel.css";

interface MeetingDetailSheetProps {
  detail: MeetingDetail;
  onBack: () => void;
  onDeleted?: () => void;
}

type ExportKind = "markdown" | "pdf" | "json";

type ContentLoadState<T> =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; data: T }
  | { status: "error"; message: string };

export function MeetingDetailSheet({ detail, onBack, onDeleted }: MeetingDetailSheetProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [transcriptionState, setTranscriptionState] = useState<ContentLoadState<Transcription>>({
    status: "idle",
  });
  const [summaryState, setSummaryState] = useState<ContentLoadState<SummaryRecord>>({
    status: "idle",
  });

  const audioFile = detail.audioFiles[0];
  const transcriptionMeta = detail.transcriptions[detail.transcriptions.length - 1];
  const summaryMeta = detail.summaries[detail.summaries.length - 1];
  const transcriptionId = transcriptionMeta?.id;
  const summaryId = summaryMeta?.id;
  const durationMs = meetingDurationMs(detail);

  const transcription =
    transcriptionState.status === "ready" ? transcriptionState.data : undefined;
  const summaryRecord = summaryState.status === "ready" ? summaryState.data : undefined;
  const structured = summaryRecord ? parseStoredSummary(summaryRecord.content) : null;
  const canExportReport = structured !== null;

  useEffect(() => {
    if (!transcriptionId) {
      setTranscriptionState({ status: "idle" });
      return;
    }

    let cancelled = false;
    setTranscriptionState({ status: "loading" });
    void getTranscription(transcriptionId)
      .then((data) => {
        if (!cancelled) {
          setTranscriptionState({ status: "ready", data });
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setTranscriptionState({
            status: "error",
            message: err instanceof Error ? err.message : "Chargement impossible.",
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [transcriptionId]);

  useEffect(() => {
    if (!summaryId) {
      setSummaryState({ status: "idle" });
      return;
    }

    let cancelled = false;
    setSummaryState({ status: "loading" });
    void getSummary(summaryId)
      .then((data) => {
        if (!cancelled) {
          setSummaryState({ status: "ready", data });
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setSummaryState({
            status: "error",
            message: err instanceof Error ? err.message : "Chargement impossible.",
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [summaryId]);

  async function handleExport(kind: ExportKind) {
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      const exportedAt = new Date().toISOString();

      if (kind === "json") {
        const contents = await exportMeeting(detail.id);
        const defaultPath = buildExportFilename(detail.title, exportedAt, "json");
        const path = await save({
          defaultPath,
          filters: [{ name: "JSON", extensions: ["json"] }],
        });
        if (path === null) {
          return;
        }
        await writeExportFile(path, contents);
        setStatusMessage("Export JSON enregistré.");
        return;
      }

      if (!structured) {
        setError("Aucun compte-rendu structuré à exporter.");
        return;
      }

      if (kind === "markdown") {
        const contents = buildReportMarkdown(reportExportMeta(detail), structured);
        const defaultPath = buildExportFilename(detail.title, exportedAt, "md");
        const path = await save({
          defaultPath,
          filters: [{ name: "Markdown", extensions: ["md"] }],
        });
        if (path === null) {
          return;
        }
        await writeExportFile(path, contents);
        setStatusMessage("Export Markdown enregistré.");
        return;
      }

      const pdfBase64 = await exportMeetingPdf(detail.id);
      const defaultPath = buildExportFilename(detail.title, exportedAt, "pdf");
      const path = await save({
        defaultPath,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (path === null) {
        return;
      }
      await writeExportBytes(path, pdfBase64);
      setStatusMessage("Export PDF enregistré.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Export impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete() {
    if (
      !window.confirm(
        `Supprimer définitivement la réunion « ${detail.title} » et son fichier audio ?`,
      )
    ) {
      return;
    }

    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      await deleteMeeting(detail.id);
      onDeleted?.();
      onBack();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Suppression impossible.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel meeting-detail">
      <div className="meeting-detail__header">
        <button type="button" className="meeting-detail__back" onClick={onBack}>
          ← Retour à la liste
        </button>
        <h2>{detail.title}</h2>
        <div className="row controls meeting-detail__actions">
          <button
            type="button"
            disabled={busy || !canExportReport}
            onClick={() => void handleExport("markdown")}
            title={
              canExportReport
                ? "Exporter le compte-rendu en Markdown"
                : "Aucun compte-rendu structuré à exporter"
            }
          >
            Exporter Markdown
          </button>
          <button
            type="button"
            disabled={busy || !canExportReport}
            onClick={() => void handleExport("pdf")}
            title={
              canExportReport
                ? "Exporter le compte-rendu en PDF brandé"
                : "Aucun compte-rendu structuré à exporter"
            }
          >
            Exporter PDF
          </button>
          <button type="button" disabled={busy} onClick={() => void handleExport("json")}>
            Exporter JSON
          </button>
          <button
            type="button"
            className="meeting-detail__danger"
            disabled={busy}
            onClick={() => void handleDelete()}
          >
            Supprimer
          </button>
        </div>
      </div>

      <dl className="status-grid">
        <div>
          <dt>Statut</dt>
          <dd>{meetingStatusLabel(detail.status)}</dd>
        </div>
        <div>
          <dt>Date</dt>
          <dd>{meetingDisplayDate(detail)}</dd>
        </div>
        <div>
          <dt>Durée</dt>
          <dd>{formatDurationMs(durationMs)}</dd>
        </div>
      </dl>

      {statusMessage && (
        <p className="meeting-detail__status" role="status">
          {statusMessage}
        </p>
      )}

      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}

      {audioFile && (
        <article className="meeting-detail__block">
          <h3>Audio</h3>
          <audio
            controls
            src={convertFileSrc(audioFile.filePath)}
            className="meeting-detail__audio"
          >
            Votre navigateur ne supporte pas la lecture audio.
          </audio>
        </article>
      )}

      {transcriptionMeta && (
        <article className="meeting-detail__block">
          <h3>Transcription</h3>
          {transcriptionMeta.providerId && (
            <p className="meta">Fournisseur : {transcriptionMeta.providerId}</p>
          )}
          {transcriptionState.status === "loading" && (
            <p className="progress-message">Chargement de la transcription…</p>
          )}
          {transcriptionState.status === "error" && (
            <p className="error" role="alert">
              {transcriptionState.message}
            </p>
          )}
          {transcription && (
            <div className="meeting-detail__scroll">
              <p>{transcription.content}</p>
              {transcription.language && (
                <p className="meta">Langue détectée : {transcription.language}</p>
              )}
            </div>
          )}
        </article>
      )}

      {summaryMeta && (
        <article className="meeting-detail__block structured-summary-inline">
          <h3>Compte-rendu structuré</h3>
          {summaryState.status === "loading" && (
            <p className="progress-message">Chargement du compte-rendu…</p>
          )}
          {summaryState.status === "error" && (
            <p className="error" role="alert">
              {summaryState.message}
            </p>
          )}
          {structured && (
            <StructuredSummaryView
              summary={structured}
              providerId={summaryRecord?.providerId}
              headingLevel={4}
            />
          )}
        </article>
      )}

      {detail.actions.length > 0 && (
        <article className="meeting-detail__block">
          <h3>Actions enregistrées</h3>
          <ul className="meeting-detail__actions-list">
            {detail.actions.map((action) => (
              <li key={action.id}>
                <strong>{action.title}</strong>
                {action.assignee && (
                  <span className="structured-summary__tag">{action.assignee}</span>
                )}
                {action.dueDate && (
                  <span className="structured-summary__tag">{action.dueDate}</span>
                )}
              </li>
            ))}
          </ul>
        </article>
      )}
    </section>
  );
}
