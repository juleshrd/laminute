import { describe, expect, it } from "vitest";

import {
  defaultRecordingTitle,
  durationFromMeetingDetail,
  isMp3Path,
  isTranscriptionBusy,
  meetingFlowStatusLabel,
  recordingPhaseToFlowPhase,
  transcriptionPhaseLabel,
} from "./meetingFlow";

describe("meetingFlow helpers", () => {
  it("maps recording phases to flow phases", () => {
    expect(recordingPhaseToFlowPhase("idle")).toBe("idle");
    expect(recordingPhaseToFlowPhase("recording")).toBe("recording");
    expect(recordingPhaseToFlowPhase("stopped")).toBe("ready");
  });

  it("returns French labels for each flow phase", () => {
    expect(meetingFlowStatusLabel("idle")).toMatch(/enregistrer/i);
    expect(meetingFlowStatusLabel("recording")).toMatch(/enregistrement/i);
    expect(meetingFlowStatusLabel("ready")).toMatch(/traiter/i);
    expect(meetingFlowStatusLabel("processing")).toMatch(/traitement/i);
    expect(meetingFlowStatusLabel("done")).toMatch(/consultation/i);
    expect(meetingFlowStatusLabel("error")).toMatch(/erreur/i);
  });

  it("maps transcription phases to French labels", () => {
    expect(transcriptionPhaseLabel("uploading")).toMatch(/envoi/i);
    expect(transcriptionPhaseLabel("idle")).toBeNull();
  });

  it("detects busy transcription phases", () => {
    expect(isTranscriptionBusy("transcribing")).toBe(true);
    expect(isTranscriptionBusy("completed")).toBe(false);
    expect(isTranscriptionBusy("failed")).toBe(false);
  });

  it("formats default recording titles", () => {
    const title = defaultRecordingTitle(new Date("2026-08-05T14:30:00"));
    expect(title).toBe("Enregistrement 05/08/2026 14:30");
  });

  it("converts meeting duration from milliseconds", () => {
    expect(durationFromMeetingDetail(65_000)).toBe(65);
    expect(durationFromMeetingDetail(null)).toBeNull();
  });

  it("detects mp3 paths", () => {
    expect(isMp3Path("/tmp/audio.MP3")).toBe(true);
    expect(isMp3Path("/tmp/audio.wav")).toBe(false);
  });
});
