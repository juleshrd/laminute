import type { RecordingPhase, RecordingStatus } from "./audio";
import type { TranscriptionPhase, TranscriptionProgress } from "./transcription";

export type MeetingFlowPhase = "idle" | "recording" | "ready" | "processing" | "done" | "error";

export interface HydratedMeetingFlow {
  flowPhase: MeetingFlowPhase;
  filePath: string | null;
  durationSecs: number | null;
  title: string | null;
  processingStep: "transcribing" | "summarizing" | null;
  transcriptionProgress: TranscriptionProgress | null;
  meetingId: string | null;
}

const FLOW_STATUS_LABELS: Record<MeetingFlowPhase, string> = {
  idle: "Prêt à enregistrer ?",
  recording: "Enregistrement en cours",
  ready: "Audio prêt — vous pouvez traiter la réunion",
  processing: "Traitement en cours…",
  done: "Réunion traitée — consultation du compte-rendu",
  error: "Une erreur est survenue pendant le traitement",
};

const TRANSCRIPTION_PHASE_LABELS: Partial<Record<TranscriptionPhase, string>> = {
  preparing: "Préparation de la transcription…",
  uploading: "Envoi de l'audio au service de transcription…",
  transcribing: "Transcription en cours…",
  saving: "Enregistrement de la transcription…",
  completed: "Transcription terminée",
  failed: "Échec de la transcription",
};

export function meetingFlowStatusLabel(phase: MeetingFlowPhase): string {
  return FLOW_STATUS_LABELS[phase];
}

export function transcriptionPhaseLabel(phase: TranscriptionPhase): string | null {
  return TRANSCRIPTION_PHASE_LABELS[phase] ?? null;
}

export function isTranscriptionBusy(phase: TranscriptionPhase): boolean {
  return (
    phase === "preparing" || phase === "uploading" || phase === "transcribing" || phase === "saving"
  );
}

export function recordingPhaseToFlowPhase(phase: RecordingPhase): "idle" | "recording" | "ready" {
  if (phase === "recording") {
    return "recording";
  }
  if (phase === "stopped") {
    return "ready";
  }
  return "idle";
}

/** Reconstruit l'état UI depuis les services natifs (enregistrement / transcription). */
export function hydrateMeetingFlowFromNative(input: {
  recording: RecordingStatus | null;
  transcription: TranscriptionProgress | null;
}): HydratedMeetingFlow | null {
  const { recording, transcription } = input;

  if (recording?.phase === "recording") {
    return {
      flowPhase: "recording",
      filePath: recording.filePath,
      durationSecs: recording.durationSecs,
      title: defaultRecordingTitle(),
      processingStep: null,
      transcriptionProgress: null,
      meetingId: null,
    };
  }

  if (transcription && isTranscriptionBusy(transcription.phase)) {
    return {
      flowPhase: "processing",
      filePath: null,
      durationSecs: null,
      title: null,
      processingStep: "transcribing",
      transcriptionProgress: transcription,
      meetingId: transcription.meetingId ?? null,
    };
  }

  return null;
}

export function defaultRecordingTitle(now: Date = new Date()): string {
  const pad = (value: number) => value.toString().padStart(2, "0");
  return `Enregistrement ${pad(now.getDate())}/${pad(now.getMonth() + 1)}/${now.getFullYear()} ${pad(now.getHours())}:${pad(now.getMinutes())}`;
}

export function durationFromMeetingDetail(durationMs: number | null | undefined): number | null {
  if (durationMs === null || durationMs === undefined) {
    return null;
  }
  return Math.round(durationMs / 1000);
}

export function isMp3Path(path: string): boolean {
  return path.toLowerCase().endsWith(".mp3");
}
