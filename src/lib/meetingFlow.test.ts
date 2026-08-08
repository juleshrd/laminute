import { describe, expect, it } from "vitest";

import {
  defaultRecordingTitle,
  durationFromMeetingDetail,
  hydrateMeetingFlowFromNative,
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

  it("hydrates recording flow from native status", () => {
    const hydrated = hydrateMeetingFlowFromNative({
      recording: {
        phase: "recording",
        deviceId: "mic-1",
        filePath: null,
        durationSecs: 42,
        error: null,
      },
      transcription: null,
    });

    expect(hydrated?.flowPhase).toBe("recording");
    expect(hydrated?.durationSecs).toBe(42);
    expect(hydrated?.title).toMatch(/^Enregistrement /);
  });

  it("hydrates busy transcription without starting a new job", () => {
    const hydrated = hydrateMeetingFlowFromNative({
      recording: {
        phase: "idle",
        deviceId: null,
        filePath: null,
        durationSecs: null,
        error: null,
      },
      transcription: {
        phase: "transcribing",
        message: "Transcription en cours…",
        meetingId: "meeting-1",
      },
    });

    expect(hydrated?.flowPhase).toBe("processing");
    expect(hydrated?.processingStep).toBe("transcribing");
    expect(hydrated?.meetingId).toBe("meeting-1");
  });

  it("prefers active recording over transcription progress", () => {
    const hydrated = hydrateMeetingFlowFromNative({
      recording: {
        phase: "recording",
        deviceId: "mic-1",
        filePath: null,
        durationSecs: 3,
        error: null,
      },
      transcription: {
        phase: "uploading",
        message: "Envoi…",
      },
    });

    expect(hydrated?.flowPhase).toBe("recording");
  });

  it("returns null when nothing is active", () => {
    expect(
      hydrateMeetingFlowFromNative({
        recording: {
          phase: "idle",
          deviceId: null,
          filePath: null,
          durationSecs: null,
          error: null,
        },
        transcription: null,
      }),
    ).toBeNull();
  });
});
