import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import { getAiSettings, listAiProviders } from "../lib/ai/api";
import { generateStructuredSummary } from "../lib/ai/api";
import type { GenerateStructuredSummaryOutput, ProviderInfo } from "../lib/ai/types";
import "../components/StructuredSummaryPanel.css";
import { ClockMark } from "./ClockMark";
import { DataProcessingNotice } from "./DataProcessingNotice";
import { RecordingConsentModal } from "./RecordingConsentModal";
import {
  type AudioInputDevice,
  type MeetingDetail,
  type RecordingStatus,
  formatAudioError,
  formatDuration,
  updateMeetingTitle,
} from "../lib/audio";
import {
  type MeetingFlowPhase,
  defaultRecordingTitle,
  durationFromMeetingDetail,
  isMp3Path,
  isTranscriptionBusy,
  meetingFlowStatusLabel,
  transcriptionPhaseLabel,
} from "../lib/meetingFlow";
import {
  getTranscriptionProgress,
  listenTranscriptionProgress,
  transcribeAudioFile,
  type Transcription,
  type TranscriptionProgress,
} from "../lib/transcription";

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return formatAudioError(error);
}

export function MeetingWorkspace() {
  const [flowPhase, setFlowPhase] = useState<MeetingFlowPhase>("idle");
  const [devices, setDevices] = useState<AudioInputDevice[]>([]);
  const [selectedDeviceId, setSelectedDeviceId] = useState("");
  const [recordingStatus, setRecordingStatus] = useState<RecordingStatus | null>(null);
  const [meetingId, setMeetingId] = useState<string | null>(null);
  const [filePath, setFilePath] = useState<string | null>(null);
  const [durationSecs, setDurationSecs] = useState<number | null>(null);
  const [title, setTitle] = useState("");
  const [hasApiKey, setHasApiKey] = useState(false);
  const [transcription, setTranscription] = useState<Transcription | null>(null);
  const [summary, setSummary] = useState<GenerateStructuredSummaryOutput | null>(null);
  const [transcriptionProgress, setTranscriptionProgress] = useState<TranscriptionProgress | null>(
    null,
  );
  const [processingStep, setProcessingStep] = useState<"transcribing" | "summarizing" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [importing, setImporting] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const [showRecordingConsent, setShowRecordingConsent] = useState(false);
  const [providerName, setProviderName] = useState("Mistral");
  const [selectedProvider, setSelectedProvider] = useState<ProviderInfo | null>(null);
  const [pastedText, setPastedText] = useState("");

  const refreshDevices = useCallback(async () => {
    try {
      const listed = await invoke<AudioInputDevice[]>("list_audio_input_devices");
      setDevices(listed);

      const selected = await invoke<AudioInputDevice | null>("get_selected_audio_input_device");
      if (selected) {
        setSelectedDeviceId(selected.id);
      } else if (listed.length > 0) {
        const fallback = listed.find((device) => device.isDefault) ?? listed[0];
        setSelectedDeviceId(fallback.id);
      }
    } catch (err) {
      setError(formatError(err));
    }
  }, []);

  const refreshRecordingStatus = useCallback(async () => {
    try {
      const status = await invoke<RecordingStatus>("get_recording_status");
      setRecordingStatus(status);
      return status;
    } catch (err) {
      setError(formatError(err));
      return null;
    }
  }, []);

  const refreshAiSettings = useCallback(async () => {
    try {
      const [settings, providers] = await Promise.all([getAiSettings(), listAiProviders()]);
      setHasApiKey(settings.hasApiKey);
      const selected = providers.find((provider) => provider.id === settings.selectedProviderId);
      setSelectedProvider(selected ?? providers[0] ?? null);
      setProviderName(selected?.displayName ?? providers[0]?.displayName ?? "Mistral");
    } catch (err) {
      setError(formatError(err));
    }
  }, []);

  useEffect(() => {
    void (async () => {
      setLoading(true);
      await Promise.all([refreshDevices(), refreshRecordingStatus(), refreshAiSettings()]);
      setLoading(false);
    })();
  }, [refreshAiSettings, refreshDevices, refreshRecordingStatus]);

  useEffect(() => {
    if (flowPhase !== "recording") {
      return;
    }

    const timer = window.setInterval(() => {
      void refreshRecordingStatus();
    }, 500);

    return () => window.clearInterval(timer);
  }, [flowPhase, refreshRecordingStatus]);

  useEffect(() => {
    if (flowPhase !== "processing") {
      return;
    }

    void getTranscriptionProgress()
      .then(setTranscriptionProgress)
      .catch(() => undefined);

    let unlisten: (() => void) | undefined;
    void listenTranscriptionProgress((progress) => {
      setTranscriptionProgress(progress);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [flowPhase]);

  const importMp3 = useCallback(async (sourcePath: string) => {
    if (!isMp3Path(sourcePath)) {
      setError("Seuls les fichiers MP3 sont acceptés.");
      return;
    }

    setImporting(true);
    setError(null);

    try {
      const detail = await invoke<MeetingDetail>("import_mp3_meeting", { sourcePath });
      const audioFile = detail.audioFiles[0];
      setMeetingId(detail.id);
      setFilePath(audioFile?.filePath ?? null);
      setDurationSecs(durationFromMeetingDetail(audioFile?.durationMs));
      setTitle(detail.title);
      setTranscription(null);
      setSummary(null);
      setFlowPhase("ready");
    } catch (err) {
      setError(formatError(err));
    } finally {
      setImporting(false);
      setDragOver(false);
    }
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    void listen<{ paths: string[] }>("tauri://drag-drop", (event) => {
      if (!active || importing || flowPhase === "processing" || flowPhase === "recording") {
        return;
      }

      const mp3Path = event.payload.paths.find((path) => isMp3Path(path));
      if (!mp3Path) {
        setError("Déposez un fichier MP3 valide.");
        setDragOver(false);
        return;
      }

      void importMp3(mp3Path);
    }).then((dispose) => {
      if (!active) {
        dispose();
        return;
      }
      unlisten = dispose;
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [flowPhase, importMp3, importing]);

  async function handleStartRecording() {
    setError(null);
    try {
      const status = await invoke<RecordingStatus>("start_microphone_recording");
      setRecordingStatus(status);
      setMeetingId(null);
      setFilePath(null);
      setDurationSecs(null);
      setTitle(defaultRecordingTitle());
      setTranscription(null);
      setSummary(null);
      setFlowPhase("recording");
      setShowRecordingConsent(false);
    } catch (err) {
      setError(formatError(err));
      await refreshRecordingStatus();
    }
  }

  function requestStartRecording() {
    setShowRecordingConsent(true);
  }

  async function handleStopRecording() {
    setError(null);
    try {
      const status = await invoke<RecordingStatus>("stop_microphone_recording");
      setRecordingStatus(status);
      setFilePath(status.filePath);
      setDurationSecs(status.durationSecs);
      const nextTitle = title.trim() || defaultRecordingTitle();
      setTitle(nextTitle);

      const providerCanTranscribe = selectedProvider?.capabilities.transcription ?? true;
      if (status.filePath && hasApiKey && providerCanTranscribe) {
        await runProcessing({
          filePath: status.filePath,
          durationSecs: status.durationSecs,
          meetingTitle: nextTitle,
        });
        return;
      }

      setFlowPhase("ready");
    } catch (err) {
      setError(formatError(err));
      await refreshRecordingStatus();
    }
  }

  async function handlePickMp3() {
    setError(null);

    const selected = await open({
      multiple: false,
      filters: [{ name: "MP3", extensions: ["mp3"] }],
    });

    if (selected === null) {
      return;
    }

    const sourcePath = Array.isArray(selected) ? selected[0] : selected;
    if (!sourcePath) {
      return;
    }

    await importMp3(sourcePath);
  }

  async function persistTitleIfNeeded(nextTitle: string) {
    if (!meetingId) {
      return;
    }

    const trimmed = nextTitle.trim();
    if (!trimmed) {
      return;
    }

    try {
      await updateMeetingTitle(meetingId, trimmed);
    } catch (err) {
      setError(formatError(err));
    }
  }

  async function runProcessing(overrides?: {
    filePath?: string | null;
    durationSecs?: number | null;
    meetingTitle?: string;
  }) {
    const activeFilePath = overrides?.filePath ?? filePath;
    const activeDurationSecs = overrides?.durationSecs ?? durationSecs;
    const activeTitle = overrides?.meetingTitle ?? title;

    if (!activeFilePath) {
      return;
    }

    const providerCanTranscribe = selectedProvider?.capabilities.transcription ?? true;

    setFlowPhase("processing");
    setError(null);

    try {
      let nextTranscription = transcription;

      if (!nextTranscription && providerCanTranscribe) {
        setProcessingStep("transcribing");
        nextTranscription = await transcribeAudioFile({
          filePath: activeFilePath,
          meetingId: meetingId ?? undefined,
          meetingTitle: meetingId ? undefined : activeTitle.trim() || undefined,
          language: "fr",
          durationMs:
            activeDurationSecs != null ? Math.round(activeDurationSecs * 1000) : undefined,
        });
        setTranscription(nextTranscription);
        setMeetingId(nextTranscription.meetingId);
      }

      const summaryText = nextTranscription?.content ?? pastedText.trim();
      if (!summaryText) {
        throw new Error("Aucun texte disponible pour générer le compte-rendu.");
      }

      if (activeTitle.trim() && (nextTranscription?.meetingId ?? meetingId)) {
        await updateMeetingTitle(nextTranscription?.meetingId ?? meetingId!, activeTitle.trim());
      }

      setProcessingStep("summarizing");
      const nextSummary = await generateStructuredSummary({
        meetingId: nextTranscription?.meetingId ?? meetingId ?? undefined,
        text: nextTranscription ? undefined : summaryText,
      });
      setSummary(nextSummary);
      if (!meetingId) {
        setMeetingId(nextSummary.meetingId);
      }
      setFlowPhase("done");
    } catch (err) {
      setError(formatError(err));
      setFlowPhase("error");
    } finally {
      setProcessingStep(null);
      await refreshAiSettings();
    }
  }

  async function runSummarizeFromText() {
    const text = pastedText.trim();
    if (!text) {
      return;
    }

    setFlowPhase("processing");
    setError(null);
    setProcessingStep("summarizing");

    try {
      const nextSummary = await generateStructuredSummary({
        meetingId: meetingId ?? undefined,
        text,
      });
      setSummary(nextSummary);
      setMeetingId(nextSummary.meetingId);
      setFlowPhase("done");
    } catch (err) {
      setError(formatError(err));
      setFlowPhase("error");
    } finally {
      setProcessingStep(null);
      await refreshAiSettings();
    }
  }

  const isRecording = flowPhase === "recording";
  const showReadyControls = flowPhase === "ready" || flowPhase === "error" || flowPhase === "done";
  const progressMessage =
    processingStep === "summarizing"
      ? "Génération du compte-rendu structuré…"
      : transcriptionProgress
        ? transcriptionPhaseLabel(transcriptionProgress.phase)
        : null;
  const canTranscribe = selectedProvider?.capabilities.transcription ?? true;
  const isSummarizeOnly = !canTranscribe;
  const isBusy =
    flowPhase === "processing" ||
    (transcriptionProgress != null && isTranscriptionBusy(transcriptionProgress.phase));
  const canStartRecording = Boolean(selectedDeviceId) && devices.length > 0;

  return (
    <div className={`meeting-workspace${isRecording ? " meeting-workspace--recording" : ""}`}>
      {!isRecording && (
        <div
          className={`status-banner status-banner--${flowPhase}`}
          role="status"
          aria-live="polite"
        >
          {meetingFlowStatusLabel(flowPhase)}
        </div>
      )}

      {loading ? (
        <p>Chargement…</p>
      ) : (
        <>
          {flowPhase === "idle" && (
            <section className="meeting-hero" aria-labelledby="meeting-hero-title">
              <h2 id="meeting-hero-title">Prêt à enregistrer ?</h2>

              <button
                type="button"
                className="meeting-logo-cta"
                aria-label="Démarrer l'enregistrement"
                onClick={requestStartRecording}
                disabled={!canStartRecording}
              >
                <ClockMark className="meeting-logo-cta__mark" />
              </button>

              {devices.length === 0 ? (
                <p className="warning meeting-hero__hint">
                  Aucun périphérique d&apos;entrée détecté.
                </p>
              ) : (
                <p className="meeting-hero__hint lm-subtle">
                  Cliquez sur le logo pour démarrer la réunion.
                </p>
              )}

              <section
                className={`meeting-hero__import drop-zone${dragOver ? " drop-zone-active" : ""}`}
                onDragEnter={() => setDragOver(true)}
                onDragLeave={() => setDragOver(false)}
                onDragOver={(event) => {
                  event.preventDefault();
                  setDragOver(true);
                }}
              >
                <p className="drop-zone-hint">Ou importez un fichier MP3</p>
                <div className="row controls">
                  <button type="button" onClick={() => void handlePickMp3()} disabled={importing}>
                    {importing ? "Import en cours…" : "Choisir un fichier MP3"}
                  </button>
                </div>
                <p className="drop-zone-constraints">
                  MP3 uniquement · 500 Mo max · entre 1 s et 4 h
                </p>
              </section>
            </section>
          )}

          {flowPhase === "recording" && (
            <section className="meeting-recording-stage" aria-live="polite">
              <p className="meeting-timer" aria-label="Durée de l'enregistrement">
                {formatDuration(recordingStatus?.durationSecs ?? 0)}
              </p>
              <button
                type="button"
                className="lm-btn meeting-recording-stage__stop"
                onClick={() => void handleStopRecording()}
              >
                Terminer la réunion
              </button>
            </section>
          )}

          {showReadyControls && filePath && (
            <section className="panel">
              <h2>Réunion</h2>
              <div className="meeting-workspace__field">
                <label htmlFor="meeting-title">Titre</label>
                <input
                  id="meeting-title"
                  type="text"
                  value={title}
                  onChange={(event) => setTitle(event.target.value)}
                  onBlur={() => void persistTitleIfNeeded(title)}
                />
              </div>

              <dl className="status-grid">
                <div>
                  <dt>Durée</dt>
                  <dd>{formatDuration(durationSecs)}</dd>
                </div>
                {meetingId && (
                  <div>
                    <dt>Identifiant</dt>
                    <dd className="mono">{meetingId}</dd>
                  </div>
                )}
                <div>
                  <dt>Fichier</dt>
                  <dd className="mono">{filePath}</dd>
                </div>
              </dl>

              {!hasApiKey && (flowPhase === "ready" || flowPhase === "error") && (
                <p className="warning">
                  Configurez {isSummarizeOnly ? "la connexion" : "une clé API"} pour {providerName}{" "}
                  dans les réglages IA avant de traiter la réunion.
                </p>
              )}

              {(flowPhase === "ready" || flowPhase === "error") && hasApiKey && (
                <DataProcessingNotice
                  providerName={providerName}
                  capabilities={selectedProvider?.capabilities}
                />
              )}

              {isSummarizeOnly && (flowPhase === "ready" || flowPhase === "error") && (
                <>
                  <p className="warning" role="note">
                    {providerName} ne prend pas en charge la transcription audio. Collez le texte de
                    la réunion ci-dessous, ou choisissez OpenAI ou Mistral pour transcrire
                    automatiquement.
                  </p>
                  <div className="meeting-workspace__field">
                    <label htmlFor="pasted-transcript">Texte de la réunion</label>
                    <textarea
                      id="pasted-transcript"
                      rows={8}
                      value={pastedText}
                      disabled={isBusy}
                      onChange={(event) => setPastedText(event.target.value)}
                      placeholder="Collez ici la transcription ou les notes de la réunion…"
                    />
                  </div>
                  <div className="row controls">
                    <button
                      type="button"
                      onClick={() => void runSummarizeFromText()}
                      disabled={!hasApiKey || isBusy || !pastedText.trim()}
                    >
                      Générer le compte-rendu
                    </button>
                  </div>
                </>
              )}

              {!isSummarizeOnly && (flowPhase === "ready" || flowPhase === "error") && (
                <div className="row controls">
                  <button
                    type="button"
                    onClick={() => void runProcessing()}
                    disabled={!hasApiKey || isBusy || !title.trim()}
                  >
                    {flowPhase === "error" ? "Réessayer" : "Traiter"}
                  </button>
                </div>
              )}
            </section>
          )}

          {flowPhase === "processing" && progressMessage && (
            <p className="progress-message">{progressMessage}</p>
          )}

          {flowPhase === "done" && transcription && (
            <section className="panel">
              <h2>Transcription</h2>
              <div className="transcription-result">
                <p>{transcription.content}</p>
                {transcription.language && (
                  <p className="meta">Langue détectée : {transcription.language}</p>
                )}
              </div>
            </section>
          )}

          {flowPhase === "done" && summary && (
            <section className="panel structured-summary-inline">
              <h2>Compte-rendu structuré</h2>

              <article className="structured-summary__block">
                <h3>Synthèse</h3>
                <p>{summary.structured.synthese}</p>
              </article>

              <article className="structured-summary__block">
                <h3>Décisions</h3>
                {summary.structured.decisions.length > 0 ? (
                  <ul>
                    {summary.structured.decisions.map((decision) => (
                      <li key={decision}>{decision}</li>
                    ))}
                  </ul>
                ) : (
                  <p className="structured-summary__empty">Aucune décision identifiée.</p>
                )}
              </article>

              <article className="structured-summary__block">
                <h3>Actions</h3>
                {summary.structured.actions.length > 0 ? (
                  <ul>
                    {summary.structured.actions.map((action) => (
                      <li key={`${action.titre}-${action.responsable ?? ""}`}>
                        <strong>{action.titre}</strong>
                        {action.description && <span> — {action.description}</span>}
                        {action.responsable && (
                          <span className="structured-summary__tag">{action.responsable}</span>
                        )}
                        {action.echeance && (
                          <span className="structured-summary__tag">{action.echeance}</span>
                        )}
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="structured-summary__empty">Aucune action identifiée.</p>
                )}
              </article>
            </section>
          )}
        </>
      )}

      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}

      {showRecordingConsent && (
        <RecordingConsentModal
          onConfirm={() => void handleStartRecording()}
          onCancel={() => setShowRecordingConsent(false)}
        />
      )}
    </div>
  );
}
