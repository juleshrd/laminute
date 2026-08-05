import type { RecordingPhase } from "./audio";
import type { TranscriptionPhase } from "./transcription";

export type MeetingFlowPhase =
  | "idle"
  | "recording"
  | "ready"
  | "processing"
  | "done"
  | "error";

const FLOW_STATUS_LABELS: Record<MeetingFlowPhase, string> = {
  idle: "Prêt à enregistrer ou importer un fichier audio",
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
    phase === "preparing" ||
    phase === "uploading" ||
    phase === "transcribing" ||
    phase === "saving"
  );
}

export function recordingPhaseToFlowPhase(
  phase: RecordingPhase,
): "idle" | "recording" | "ready" {
  if (phase === "recording") {
    return "recording";
  }
  if (phase === "stopped") {
    return "ready";
  }
  return "idle";
}

export function defaultRecordingTitle(now: Date = new Date()): string {
  const pad = (value: number) => value.toString().padStart(2, "0");
  return `Enregistrement ${pad(now.getDate())}/${pad(now.getMonth() + 1)}/${now.getFullYear()} ${pad(now.getHours())}:${pad(now.getMinutes())}`;
}

export function durationFromMeetingDetail(
  durationMs: number | null | undefined,
): number | null {
  if (durationMs === null || durationMs === undefined) {
    return null;
  }
  return Math.round(durationMs / 1000);
}

export function isMp3Path(path: string): boolean {
  return path.toLowerCase().endsWith(".mp3");
}
