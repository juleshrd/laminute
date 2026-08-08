import { invoke } from "@tauri-apps/api/core";

import type { Action, SummaryRecord } from "./ai/types";

export { parseStoredSummary } from "./ai/parseStructuredSummary";
import type { AudioFile } from "./audio";
import type { Transcription } from "./transcription";

export type MeetingStatus = "draft" | "recording" | "processing" | "completed";

export interface MeetingSummary {
  id: string;
  title: string;
  status: MeetingStatus;
  startedAt: string | null;
  endedAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface MeetingListItem extends MeetingSummary {
  snippet?: string | null;
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
  transcriptions: Transcription[];
  summaries: SummaryRecord[];
  actions: Action[];
}

export interface MeetingSearchFilters {
  query?: string;
  status?: MeetingStatus;
  providerId?: string;
  dateFrom?: string;
  dateTo?: string;
  cursor?: string;
}

export interface MeetingSearchPage {
  items: MeetingListItem[];
  nextCursor: string | null;
}

const STATUS_LABELS: Record<MeetingStatus, string> = {
  draft: "Brouillon",
  recording: "Enregistrement",
  processing: "Traitement",
  completed: "Terminée",
};

export function meetingStatusLabel(status: MeetingStatus): string {
  return STATUS_LABELS[status];
}

export function listMeetings(): Promise<MeetingSummary[]> {
  return invoke<MeetingSummary[]>("list_meetings");
}

export function getMeeting(id: string): Promise<MeetingDetail> {
  return invoke<MeetingDetail>("get_meeting", { id });
}

export function searchMeetings(filters: MeetingSearchFilters): Promise<MeetingSearchPage> {
  return invoke<MeetingSearchPage>("search_meetings", { filters });
}

export function deleteMeeting(id: string): Promise<void> {
  return invoke<void>("delete_meeting", { id });
}

export function updateMeetingTitle(id: string, title: string): Promise<MeetingSummary> {
  return invoke<MeetingSummary>("update_meeting_title", { id, title });
}

export function meetingDisplayDate(
  meeting: Pick<MeetingSummary, "startedAt" | "createdAt">,
): string {
  const raw = meeting.startedAt ?? meeting.createdAt;
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) {
    return raw;
  }
  return date.toLocaleDateString("fr-FR", {
    day: "numeric",
    month: "long",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function meetingDurationMs(detail: MeetingDetail): number | null {
  const audio = detail.audioFiles[0];
  if (audio?.durationMs != null) {
    return audio.durationMs;
  }
  if (detail.startedAt && detail.endedAt) {
    const start = new Date(detail.startedAt).getTime();
    const end = new Date(detail.endedAt).getTime();
    if (!Number.isNaN(start) && !Number.isNaN(end) && end >= start) {
      return end - start;
    }
  }
  return null;
}

export function formatDurationMs(durationMs: number | null): string {
  if (durationMs === null || Number.isNaN(durationMs)) {
    return "—";
  }
  const totalSeconds = Math.floor(durationMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}
