import type { Transcription, TranscriptionSegment } from "./transcription";

export type SpeakerMap = Record<string, string>;

export function uniqueSpeakersFromSegments(
  segments: TranscriptionSegment[] | undefined,
): string[] {
  if (!segments?.length) {
    return [];
  }
  const speakers = new Set<string>();
  for (const segment of segments) {
    if (segment.speaker?.trim()) {
      speakers.add(segment.speaker);
    }
  }
  return [...speakers].sort();
}

export function displaySpeakerLabel(speakerId: string, speakerMap: SpeakerMap): string {
  const mapped = speakerMap[speakerId]?.trim();
  return mapped || speakerId;
}

export function substituteSpeakerLabels(text: string, speakerMap: SpeakerMap): string {
  if (!text || Object.keys(speakerMap).length === 0) {
    return text;
  }
  let result = text;
  const keys = Object.keys(speakerMap).sort((a, b) => b.length - a.length);
  for (const key of keys) {
    const name = speakerMap[key]?.trim();
    if (name) {
      result = result.split(key).join(name);
    }
  }
  return result;
}

export function formatTranscriptionDisplay(
  transcription: Transcription,
  speakerMap: SpeakerMap,
): string {
  if (!transcription.segments?.length) {
    return transcription.content;
  }

  return transcription.segments
    .map((segment) => {
      const text = segment.text.trim();
      if (!text) {
        return "";
      }
      const speaker = segment.speaker
        ? displaySpeakerLabel(segment.speaker, speakerMap)
        : "Locuteur";
      if (segment.start != null && segment.end != null) {
        return `[${speaker} ${segment.start.toFixed(1)}s–${segment.end.toFixed(1)}s] ${text}`;
      }
      return `[${speaker}] ${text}`;
    })
    .filter(Boolean)
    .join("\n");
}
