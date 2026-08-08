import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import { generateStructuredSummary, getAiSettings, listAiProviders } from "../lib/ai/api";
import type { GenerateStructuredSummaryOutput, ProviderInfo } from "../lib/ai/types";
import { isAudioError, type AudioInputDevice, type RecordingStatus } from "../lib/audio";
import { type MeetingDetail, updateMeetingTitle } from "../lib/meetings";
import {
  type MeetingFlowPhase,
  defaultRecordingTitle,
  durationFromMeetingDetail,
  hydrateMeetingFlowFromNative,
  isMp3Path,
  isTranscriptionBusy,
  transcriptionPhaseLabel,
} from "../lib/meetingFlow";
import {
  getTranscriptionProgress,
  listenTranscriptionProgress,
  transcribeAudioFile,
  type Transcription,
  type TranscriptionProgress,
} from "../lib/transcription";
import { formatMeetingError } from "./formatMeetingError";

export interface UseMeetingFlowResult {
  flowPhase: MeetingFlowPhase;
  loading: boolean;
  error: string | null;
  importing: boolean;
  dragOver: boolean;
  showRecordingConsent: boolean;
  devices: AudioInputDevice[];
  recordingStatus: RecordingStatus | null;
  meetingId: string | null;
  filePath: string | null;
  durationSecs: number | null;
  title: string;
  hasApiKey: boolean;
  ollamaBaseUrl: string | null;
  transcription: Transcription | null;
  summary: GenerateStructuredSummaryOutput | null;
  transcriptionProgress: TranscriptionProgress | null;
  processingStep: "transcribing" | "summarizing" | null;
  providerName: string;
  selectedProvider: ProviderInfo | null;
  pastedText: string;
  isRecording: boolean;
  showReadyControls: boolean;
  progressMessage: string | null;
  canTranscribe: boolean;
  isSummarizeOnly: boolean;
  isBusy: boolean;
  canStartRecording: boolean;
  setTitle: (title: string) => void;
  setPastedText: (text: string) => void;
  setDragOver: (dragOver: boolean) => void;
  setShowRecordingConsent: (show: boolean) => void;
  requestStartRecording: () => void;
  handleStartRecording: () => Promise<void>;
  handleStopRecording: () => Promise<void>;
  handlePickMp3: () => Promise<void>;
  persistTitleIfNeeded: (nextTitle: string) => Promise<void>;
  runProcessing: (overrides?: {
    filePath?: string | null;
    durationSecs?: number | null;
    meetingTitle?: string;
  }) => Promise<void>;
  runSummarizeFromText: () => Promise<void>;
}

function createAiJobId(prefix: "transcription" | "summary"): string {
  if (globalThis.crypto?.randomUUID) {
    return `${prefix}-${globalThis.crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function isNoInputDeviceError(error: unknown): boolean {
  if (isAudioError(error) && error.code === "no_input_device") {
    return true;
  }
  return formatMeetingError(error).toLowerCase().includes("aucun périphérique d'entrée audio");
}

export function useMeetingFlow(): UseMeetingFlowResult {
  const [flowPhase, setFlowPhase] = useState<MeetingFlowPhase>("idle");
  const [devices, setDevices] = useState<AudioInputDevice[]>([]);
  const [selectedDeviceId, setSelectedDeviceId] = useState("");
  const [recordingStatus, setRecordingStatus] = useState<RecordingStatus | null>(null);
  const [meetingId, setMeetingId] = useState<string | null>(null);
  const [filePath, setFilePath] = useState<string | null>(null);
  const [durationSecs, setDurationSecs] = useState<number | null>(null);
  const [title, setTitle] = useState("");
  const [hasApiKey, setHasApiKey] = useState(false);
  const [ollamaBaseUrl, setOllamaBaseUrl] = useState<string | null>("http://127.0.0.1:11434");
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
  const activeTranscriptionJobIdRef = useRef<string | null>(null);
  const processingPromiseRef = useRef<Promise<void> | null>(null);

  const refreshDevices = useCallback(async () => {
    try {
      const listed = await invoke<AudioInputDevice[]>("list_audio_input_devices");
      setDevices(listed);

      const selected = await invoke<AudioInputDevice | null>("get_selected_audio_input_device");
      if (selected) {
        setSelectedDeviceId(selected.id);
      } else if (listed.length > 0) {
        const fallback = await invoke<AudioInputDevice | null>("ensure_default_audio_input_device");
        setSelectedDeviceId(fallback?.id ?? "");
      } else {
        setSelectedDeviceId("");
      }
    } catch (err) {
      // Absence de micro = état attendu (import MP3 toujours possible), pas une erreur bloquante.
      if (isNoInputDeviceError(err)) {
        setDevices([]);
        setSelectedDeviceId("");
        return;
      }
      setError(formatMeetingError(err));
    }
  }, []);

  const refreshRecordingStatus = useCallback(async () => {
    try {
      const status = await invoke<RecordingStatus>("get_recording_status");
      setRecordingStatus(status);
      if (status.phase === "stopped" && status.error) {
        setFilePath(null);
        setError(status.error);
        setFlowPhase("error");
      }
      return status;
    } catch (err) {
      setError(formatMeetingError(err));
      return null;
    }
  }, []);

  const refreshAiSettings = useCallback(async () => {
    try {
      const [settings, providers] = await Promise.all([getAiSettings(), listAiProviders()]);
      setHasApiKey(settings.hasApiKey);
      setOllamaBaseUrl(settings.ollamaBaseUrl ?? "http://127.0.0.1:11434");
      const selected = providers.find((provider) => provider.id === settings.selectedProviderId);
      setSelectedProvider(selected ?? providers[0] ?? null);
      setProviderName(selected?.displayName ?? providers[0]?.displayName ?? "Mistral");
    } catch (err) {
      setError(formatMeetingError(err));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;

    void (async () => {
      setLoading(true);
      const [, recordingStatusResult] = await Promise.all([
        refreshDevices(),
        refreshRecordingStatus(),
        refreshAiSettings(),
      ]);

      let transcriptionProgressResult: TranscriptionProgress | null = null;
      try {
        transcriptionProgressResult = await getTranscriptionProgress();
      } catch {
        transcriptionProgressResult = null;
      }

      if (cancelled) {
        return;
      }

      const hydrated = hydrateMeetingFlowFromNative({
        recording: recordingStatusResult,
        transcription: transcriptionProgressResult,
      });

      if (hydrated) {
        setFlowPhase(hydrated.flowPhase);
        if (hydrated.filePath != null) {
          setFilePath(hydrated.filePath);
        }
        if (hydrated.durationSecs != null) {
          setDurationSecs(hydrated.durationSecs);
        }
        if (hydrated.title) {
          setTitle(hydrated.title);
        }
        if (hydrated.meetingId) {
          setMeetingId(hydrated.meetingId);
        }
        if (hydrated.transcriptionProgress?.jobId) {
          activeTranscriptionJobIdRef.current = hydrated.transcriptionProgress.jobId;
        }
        setProcessingStep(hydrated.processingStep);
        setTranscriptionProgress(hydrated.transcriptionProgress);
      }

      setLoading(false);
    })();

    return () => {
      cancelled = true;
    };
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

    const jobId = activeTranscriptionJobIdRef.current;

    void getTranscriptionProgress(jobId ?? undefined)
      .then((progress) => {
        if (!progress) {
          return;
        }
        if (jobId && progress.jobId !== jobId) {
          return;
        }
        setTranscriptionProgress(progress);
      })
      .catch(() => undefined);

    let unlisten: (() => void) | undefined;
    void listenTranscriptionProgress((progress) => {
      const activeJobId = activeTranscriptionJobIdRef.current;
      if (activeJobId && progress.jobId !== activeJobId) {
        return;
      }
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
      setError(formatMeetingError(err));
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

  const handleStartRecording = useCallback(async () => {
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
      setError(formatMeetingError(err));
      await refreshRecordingStatus();
    }
  }, [refreshRecordingStatus]);

  const requestStartRecording = useCallback(() => {
    setShowRecordingConsent(true);
  }, []);

  const runProcessing = useCallback(
    async (overrides?: {
      filePath?: string | null;
      durationSecs?: number | null;
      meetingTitle?: string;
    }) => {
      if (processingPromiseRef.current) {
        return processingPromiseRef.current;
      }

      const promise = (async () => {
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
            const transcriptionJobId = createAiJobId("transcription");
            activeTranscriptionJobIdRef.current = transcriptionJobId;
            setProcessingStep("transcribing");
            const output = await transcribeAudioFile({
              jobId: transcriptionJobId,
              filePath: activeFilePath,
              meetingId: meetingId ?? undefined,
              meetingTitle: meetingId ? undefined : activeTitle.trim() || undefined,
              language: "fr",
              durationMs:
                activeDurationSecs != null ? Math.round(activeDurationSecs * 1000) : undefined,
            });
            if (activeTranscriptionJobIdRef.current !== output.jobId) {
              return;
            }
            nextTranscription = output.transcription;
            setTranscription(nextTranscription);
            setMeetingId(nextTranscription.meetingId);
          }

          const summaryText = nextTranscription?.content ?? pastedText.trim();
          if (!summaryText) {
            throw new Error("Aucun texte disponible pour générer le compte-rendu.");
          }

          if (activeTitle.trim() && (nextTranscription?.meetingId ?? meetingId)) {
            await updateMeetingTitle(
              nextTranscription?.meetingId ?? meetingId!,
              activeTitle.trim(),
            );
          }

          setProcessingStep("summarizing");
          const nextSummary = await generateStructuredSummary({
            jobId: createAiJobId("summary"),
            meetingId: nextTranscription?.meetingId ?? meetingId ?? undefined,
            text: nextTranscription ? undefined : summaryText,
          });
          setSummary(nextSummary);
          if (!meetingId) {
            setMeetingId(nextSummary.meetingId);
          }
          setFlowPhase("done");
        } catch (err) {
          setError(formatMeetingError(err));
          setFlowPhase("error");
        } finally {
          activeTranscriptionJobIdRef.current = null;
          setProcessingStep(null);
          await refreshAiSettings();
        }
      })();

      processingPromiseRef.current = promise;
      try {
        await promise;
      } finally {
        if (processingPromiseRef.current === promise) {
          processingPromiseRef.current = null;
        }
      }
    },
    [
      durationSecs,
      filePath,
      meetingId,
      pastedText,
      refreshAiSettings,
      selectedProvider?.capabilities.transcription,
      title,
      transcription,
    ],
  );

  const handleStopRecording = useCallback(async () => {
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
      setError(formatMeetingError(err));
      await refreshRecordingStatus();
    }
  }, [
    hasApiKey,
    refreshRecordingStatus,
    runProcessing,
    selectedProvider?.capabilities.transcription,
    title,
  ]);

  const handlePickMp3 = useCallback(async () => {
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
  }, [importMp3]);

  const persistTitleIfNeeded = useCallback(
    async (nextTitle: string) => {
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
        setError(formatMeetingError(err));
      }
    },
    [meetingId],
  );

  const runSummarizeFromText = useCallback(async () => {
    if (processingPromiseRef.current) {
      return processingPromiseRef.current;
    }

    const promise = (async () => {
      const text = pastedText.trim();
      if (!text) {
        return;
      }

      setFlowPhase("processing");
      setError(null);
      setProcessingStep("summarizing");

      try {
        const nextSummary = await generateStructuredSummary({
          jobId: createAiJobId("summary"),
          meetingId: meetingId ?? undefined,
          text,
        });
        setSummary(nextSummary);
        setMeetingId(nextSummary.meetingId);
        setFlowPhase("done");
      } catch (err) {
        setError(formatMeetingError(err));
        setFlowPhase("error");
      } finally {
        setProcessingStep(null);
        await refreshAiSettings();
      }
    })();

    processingPromiseRef.current = promise;
    try {
      await promise;
    } finally {
      if (processingPromiseRef.current === promise) {
        processingPromiseRef.current = null;
      }
    }
  }, [meetingId, pastedText, refreshAiSettings]);

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

  return {
    flowPhase,
    loading,
    error,
    importing,
    dragOver,
    showRecordingConsent,
    devices,
    recordingStatus,
    meetingId,
    filePath,
    durationSecs,
    title,
    hasApiKey,
    ollamaBaseUrl,
    transcription,
    summary,
    transcriptionProgress,
    processingStep,
    providerName,
    selectedProvider,
    pastedText,
    isRecording,
    showReadyControls,
    progressMessage,
    canTranscribe,
    isSummarizeOnly,
    isBusy,
    canStartRecording,
    setTitle,
    setPastedText,
    setDragOver,
    setShowRecordingConsent,
    requestStartRecording,
    handleStartRecording,
    handleStopRecording,
    handlePickMp3,
    persistTitleIfNeeded,
    runProcessing,
    runSummarizeFromText,
  };
}
