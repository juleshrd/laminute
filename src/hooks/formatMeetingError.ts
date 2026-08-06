import { formatAudioError } from "../lib/audio";

export function formatMeetingError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return formatAudioError(error);
}
