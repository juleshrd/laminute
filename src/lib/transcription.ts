import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type TranscriptionPhase =
  "idle" | "preparing" | "uploading" | "transcribing" | "saving" | "completed" | "failed";

export interface TranscriptionProgress {
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
  filePath: string;
  meetingId?: string;
  meetingTitle?: string;
  language?: string;
  durationMs?: number;
}

export function getTranscriptionProgress(): Promise<TranscriptionProgress> {
  return invoke<TranscriptionProgress>("get_transcription_progress");
}

export function transcribeAudioFile(input: TranscribeAudioInput): Promise<Transcription> {
  return invoke<Transcription>("transcribe_audio_file", { input });
}

export function listenTranscriptionProgress(
  handler: (progress: TranscriptionProgress) => void,
): Promise<UnlistenFn> {
  return listen<TranscriptionProgress>("transcription-progress", (event) => {
    handler(event.payload);
  });
}
