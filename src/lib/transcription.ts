import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type TranscriptionPhase =
  "idle" | "preparing" | "uploading" | "transcribing" | "saving" | "completed" | "failed";

export interface TranscriptionProgress {
  jobId: string;
  phase: TranscriptionPhase;
  message: string;
  meetingId?: string;
}

export interface Transcription {
  id: string;
  meetingId: string;
  audioFileId?: string;
  providerId?: string;
  content: string;
  language?: string;
  createdAt: string;
  updatedAt: string;
}

export interface TranscribeAudioInput {
  jobId?: string;
  filePath: string;
  meetingId?: string;
  meetingTitle?: string;
  language?: string;
  durationMs?: number;
}

export interface TranscribeAudioOutput {
  jobId: string;
  transcription: Transcription;
}

export function getTranscriptionProgress(jobId?: string): Promise<TranscriptionProgress | null> {
  return invoke<TranscriptionProgress | null>("get_transcription_progress", {
    jobId: jobId ?? null,
  });
}

export function transcribeAudioFile(input: TranscribeAudioInput): Promise<TranscribeAudioOutput> {
  return invoke<TranscribeAudioOutput>("transcribe_audio_file", { input });
}

export function listenTranscriptionProgress(
  handler: (progress: TranscriptionProgress) => void,
): Promise<UnlistenFn> {
  return listen<TranscriptionProgress>("transcription-progress", (event) => {
    handler(event.payload);
  });
}
