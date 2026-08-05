export type RecordingPhase = "idle" | "recording" | "stopped";

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

export function formatDuration(seconds: number | null): string {
  if (seconds === null || Number.isNaN(seconds)) {
    return "—";
  }

  const wholeSeconds = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(wholeSeconds / 60);
  const remaining = wholeSeconds % 60;
  return `${minutes}:${remaining.toString().padStart(2, "0")}`;
}
