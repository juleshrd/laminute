import { useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";

import { deleteMeeting } from "../lib/meetings";
import { buildExportFilename, saveMeetingExport } from "../lib/privacy";
import {
  formatDurationMs,
  meetingDisplayDate,
  meetingDurationMs,
  meetingStatusLabel,
  parseStoredSummary,
  type MeetingDetail,
} from "../lib/meetings";
import { StructuredSummaryView } from "./StructuredSummaryView";
import "./StructuredSummaryPanel.css";

interface MeetingDetailSheetProps {
  detail: MeetingDetail;
  onBack: () => void;
  onDeleted?: () => void;
}

type ExportKind = "markdown" | "pdf" | "json";

export function MeetingDetailSheet({ detail, onBack, onDeleted }: MeetingDetailSheetProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  const audioFile = detail.audioFiles[0];
  const transcription = detail.transcriptions[detail.transcriptions.length - 1];
  const summaryRecord = detail.summaries[detail.summaries.length - 1];
  const structured = summaryRecord ? parseStoredSummary(summaryRecord.content) : null;
  const durationMs = meetingDurationMs(detail);
  const canExportReport = structured !== null;

  async function handleExport(kind: ExportKind) {
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      if (kind !== "json" && !structured) {
        setError("Aucun compte-rendu structuré à exporter.");
        return;
      }

      const exportedAt = new Date().toISOString();
      const extension = kind === "markdown" ? "md" : kind;
      const defaultFileName = buildExportFilename(detail.title, exportedAt, extension);
      const saved = await saveMeetingExport(detail.id, kind, defaultFileName);
      if (!saved) {
        return;
      }

      const labels: Record<ExportKind, string> = {
        json: "Export JSON enregistré.",
        markdown: "Export Markdown enregistré.",
        pdf: "Export PDF enregistré.",
      };
      setStatusMessage(labels[kind]);
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

      {transcription && (
        <article className="meeting-detail__block">
          <h3>Transcription</h3>
          {transcription.providerId && (
            <p className="meta">Fournisseur : {transcription.providerId}</p>
          )}
          <div className="meeting-detail__scroll">
            <p>{transcription.content}</p>
            {transcription.language && (
              <p className="meta">Langue détectée : {transcription.language}</p>
            )}
          </div>
        </article>
      )}

      {structured && (
        <article className="meeting-detail__block structured-summary-inline">
          <h3>Compte-rendu structuré</h3>
          <StructuredSummaryView
            summary={structured}
            providerId={summaryRecord?.providerId}
            headingLevel={4}
          />
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
