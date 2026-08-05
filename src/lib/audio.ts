import { invoke } from "@tauri-apps/api/core";

export type RecordingPhase = "idle" | "recording" | "stopped";

export type MeetingStatus = "draft" | "recording" | "processing" | "completed";

export interface AudioInputDevice {
  id: string;
  name: string;
  isDefault: boolean;
}

export interface RecordingStatus {
  phase: RecordingPhase;
  deviceId: string | null;
  filePath: string | null;
  durationSecs: number | null;
  error: string | null;
}

export interface AudioFile {
  id: string;
  meetingId: string;
  filePath: string;
  durationMs: number | null;
  format: string | null;
  createdAt: string;
}

export interface Meeting {
  id: string;
  title: string;
  description: string | null;
  status: MeetingStatus;
  startedAt: string | null;
  endedAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface MeetingDetail {
  id: string;
  title: string;
  description: string | null;
  status: MeetingStatus;
  startedAt: string | null;
  endedAt: string | null;
  createdAt: string;
  updatedAt: string;
  audioFiles: AudioFile[];
  transcriptions: unknown[];
  summaries: unknown[];
  actions: unknown[];
}

export interface AudioErrorPayload {
  code: string;
  message: string;
}

export function isAudioError(value: unknown): value is AudioErrorPayload {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    "message" in value &&
    typeof (value as AudioErrorPayload).code === "string" &&
    typeof (value as AudioErrorPayload).message === "string"
  );
}

export function formatAudioError(error: unknown): string {
  if (isAudioError(error)) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

export function formatDuration(seconds: number | null): string {
  if (seconds === null || Number.isNaN(seconds)) {
    return "—";
  }

  const wholeSeconds = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(wholeSeconds / 60);
  const remaining = wholeSeconds % 60;
  return `${minutes}:${remaining.toString().padStart(2, "0")}`;
}

export function updateMeetingTitle(id: string, title: string): Promise<Meeting> {
  return invoke<Meeting>("update_meeting_title", { id, title });
}
