import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";

import { deleteMeeting, getLatestSummary, getLatestTranscription } from "../lib/meetings";
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
import type { SummaryRecord } from "../lib/ai/types";
import type { Transcription } from "../lib/transcription";
import "./StructuredSummaryPanel.css";

interface MeetingDetailSheetProps {
  detail: MeetingDetail;
  onBack: () => void;
  onDeleted?: () => void;
}

type ExportKind = "markdown" | "pdf" | "json";
type DetailTab = "essential" | "transcript" | "audio";

export function MeetingDetailSheet({ detail, onBack, onDeleted }: MeetingDetailSheetProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [tab, setTab] = useState<DetailTab>("essential");
  const [summaryRecord, setSummaryRecord] = useState<SummaryRecord | null>(null);
  const [transcription, setTranscription] = useState<Transcription | null>(null);
  const [contentLoading, setContentLoading] = useState(false);
  const [contentError, setContentError] = useState<string | null>(null);
  const contentRequestId = useRef(0);

  const audioFile = detail.audioFiles[0];
  const structured = summaryRecord ? parseStoredSummary(summaryRecord.content) : null;
  const durationMs = meetingDurationMs(detail);
  const canExportReport = structured !== null;
  const providerHint =
    summaryRecord?.providerId ??
    detail.summaries[detail.summaries.length - 1]?.providerId ??
    transcription?.providerId ??
    detail.transcriptions[detail.transcriptions.length - 1]?.providerId;

  useEffect(() => {
    const requestId = ++contentRequestId.current;
    setContentError(null);
    setSummaryRecord(null);
    setTranscription(null);

    if (tab === "audio") {
      setContentLoading(false);
      return;
    }

    setContentLoading(true);
    const request =
      tab === "essential" ? getLatestSummary(detail.id) : getLatestTranscription(detail.id);
    void request
      .then((content) => {
        if (requestId !== contentRequestId.current) return;
        if (tab === "essential") setSummaryRecord(content as SummaryRecord | null);
        else setTranscription(content as Transcription | null);
      })
      .catch((err) => {
        if (requestId === contentRequestId.current) {
          setContentError(err instanceof Error ? err.message : "Chargement impossible.");
        }
      })
      .finally(() => {
        if (requestId === contentRequestId.current) setContentLoading(false);
      });
  }, [detail.id, tab]);

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
    <section className="meeting-detail meeting-result">
      <button type="button" className="meeting-detail__back" onClick={onBack}>
        ‹ Historique
      </button>

      <header className="meeting-result__head">
        <p className="lm-kicker">
          {meetingDisplayDate(detail).toUpperCase()}
          {durationMs != null ? ` · ${formatDurationMs(durationMs)}` : ""}
        </p>
        <h2>{detail.title}</h2>
        <p className="today-view__lead">
          {meetingStatusLabel(detail.status)}
          {providerHint ? ` · ${providerHint}` : ""}
        </p>
      </header>

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
      {contentError && (
        <p className="error" role="alert">
          {contentError}
        </p>
      )}

      <div className="lm-tabs" role="tablist" aria-label="Contenu de la réunion">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "essential"}
          className={tab === "essential" ? "is-active" : undefined}
          onClick={() => setTab("essential")}
        >
          Essentiel
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "transcript"}
          className={tab === "transcript" ? "is-active" : undefined}
          onClick={() => setTab("transcript")}
        >
          Transcription
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "audio"}
          className={tab === "audio" ? "is-active" : undefined}
          onClick={() => setTab("audio")}
        >
          Audio
        </button>
      </div>

      {tab === "essential" ? (
        <div className="meeting-result__panel" role="tabpanel">
          {contentLoading ? (
            <p className="lm-subtle">Chargement du compte-rendu…</p>
          ) : structured ? (
            <>
              <article className="essential-summary">
                <p className="lm-kicker">En une phrase</p>
                <p>{structured.synthese}</p>
              </article>
              <div className="essential-grid">
                <div>
                  <h3>Décisions</h3>
                  {structured.decisions.length > 0 ? (
                    structured.decisions.map((decision) => (
                      <article key={decision} className="essential-card">
                        <b>{decision}</b>
                      </article>
                    ))
                  ) : (
                    <p className="lm-subtle">Aucune décision identifiée.</p>
                  )}
                </div>
                <div>
                  <h3>Actions</h3>
                  {structured.actions.length > 0 ? (
                    structured.actions.map((action) => (
                      <article
                        key={`${action.titre}-${action.responsable ?? ""}`}
                        className="essential-card"
                      >
                        <b>{action.titre}</b>
                        {(action.responsable || action.echeance) && (
                          <span>
                            {[action.responsable, action.echeance].filter(Boolean).join(" · ")}
                          </span>
                        )}
                      </article>
                    ))
                  ) : detail.actions.length > 0 ? (
                    detail.actions.map((action) => (
                      <article key={action.id} className="essential-card">
                        <b>{action.title}</b>
                        {(action.assignee || action.dueDate) && (
                          <span>
                            {[action.assignee, action.dueDate].filter(Boolean).join(" · ")}
                          </span>
                        )}
                      </article>
                    ))
                  ) : (
                    <p className="lm-subtle">Aucune action identifiée.</p>
                  )}
                </div>
              </div>
              <details className="meeting-result__more">
                <summary>Voir le compte-rendu complet</summary>
                <StructuredSummaryView
                  summary={structured}
                  providerId={summaryRecord?.providerId}
                  headingLevel={4}
                />
              </details>
            </>
          ) : (
            <p className="lm-subtle">Aucun compte-rendu structuré pour cette réunion.</p>
          )}
        </div>
      ) : null}

      {tab === "transcript" ? (
        <div className="meeting-result__panel" role="tabpanel">
          {contentLoading ? (
            <p className="lm-subtle">Chargement de la transcription…</p>
          ) : transcription ? (
            <div className="meeting-detail__scroll">
              {transcription.providerId ? (
                <p className="meta">Fournisseur : {transcription.providerId}</p>
              ) : null}
              <p>{transcription.content}</p>
              {transcription.language ? (
                <p className="meta">Langue détectée : {transcription.language}</p>
              ) : null}
            </div>
          ) : (
            <p className="lm-subtle">Aucune transcription disponible.</p>
          )}
        </div>
      ) : null}

      {tab === "audio" ? (
        <div className="meeting-result__panel" role="tabpanel">
          {audioFile ? (
            <audio
              controls
              src={convertFileSrc(audioFile.filePath)}
              className="meeting-detail__audio"
            >
              Votre navigateur ne supporte pas la lecture audio.
            </audio>
          ) : (
            <p className="lm-subtle">Aucun fichier audio disponible.</p>
          )}
        </div>
      ) : null}
    </section>
  );
}
