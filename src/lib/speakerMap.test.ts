import { describe, expect, it } from "vitest";

import {
  displaySpeakerLabel,
  formatTranscriptionDisplay,
  substituteSpeakerLabels,
  uniqueSpeakersFromSegments,
} from "./speakerMap";
import type { Transcription } from "./transcription";

describe("speakerMap", () => {
  it("lists unique speakers from segments", () => {
    expect(
      uniqueSpeakersFromSegments([
        { speaker: "SPEAKER_01", text: "Bonjour" },
        { speaker: "SPEAKER_00", text: "Salut" },
        { speaker: "SPEAKER_01", text: "Au revoir" },
      ]),
    ).toEqual(["SPEAKER_00", "SPEAKER_01"]);
  });

  it("substitutes technical labels in text", () => {
    const map = { SPEAKER_00: "Marie" };
    expect(substituteSpeakerLabels("Action pour SPEAKER_00", map)).toBe("Action pour Marie");
    expect(displaySpeakerLabel("SPEAKER_00", map)).toBe("Marie");
    expect(displaySpeakerLabel("SPEAKER_01", map)).toBe("SPEAKER_01");
  });

  it("formats transcription display with mapped speaker names", () => {
    const transcription: Transcription = {
      id: "tx-1",
      meetingId: "m-1",
      content: "[SPEAKER_00] Bonjour",
      segments: [{ speaker: "SPEAKER_00", text: "Bonjour", start: 0, end: 1.2 }],
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    };

    expect(formatTranscriptionDisplay(transcription, { SPEAKER_00: "Marie" })).toContain(
      "[Marie 0.0s–1.2s] Bonjour",
    );
  });
});
