import { describe, expect, it, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { useMeetingFlow } from "./useMeetingFlow";

const invokeMock = vi.fn();
const listenMock = vi.fn().mockResolvedValue(() => undefined);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("../lib/ai/api", () => ({
  getAiSettings: vi.fn().mockResolvedValue({
    hasApiKey: true,
    selectedProviderId: "mistral",
    ollamaAllowRemote: false,
    diarizationEnabled: false,
    transcriptionModel: "voxtral-mini-latest",
    summaryModel: "mistral-small-latest",
    transcriptionModels: [],
    summaryModels: [],
  }),
  listAiProviders: vi.fn().mockResolvedValue([
    {
      id: "mistral",
      displayName: "Mistral AI",
      capabilities: {
        transcription: true,
        summary: true,
        local: false,
        streaming: false,
        diarization: true,
      },
    },
  ]),
  generateStructuredSummary: vi.fn(),
}));

vi.mock("../lib/transcription", async () => {
  const actual =
    await vi.importActual<typeof import("../lib/transcription")>("../lib/transcription");
  return {
    ...actual,
    getTranscriptionProgress: vi.fn().mockResolvedValue(null),
    listenTranscriptionProgress: vi.fn().mockResolvedValue(() => undefined),
    transcribeAudioFile: vi.fn(),
  };
});

function setupDefaultInvoke() {
  invokeMock.mockImplementation((command: string) => {
    switch (command) {
      case "list_audio_input_devices":
        return Promise.resolve([{ id: "mic-1", name: "Micro intégré", isDefault: true }]);
      case "get_selected_audio_input_device":
        return Promise.resolve({ id: "mic-1", name: "Micro intégré", isDefault: true });
      case "get_recording_status":
        return Promise.resolve({
          phase: "idle",
          deviceId: "mic-1",
          filePath: null,
          durationSecs: null,
          error: null,
        });
      default:
        return Promise.resolve(null);
    }
  });
}

describe("useMeetingFlow", () => {
  beforeEach(async () => {
    invokeMock.mockReset();
    listenMock.mockClear();
    setupDefaultInvoke();
    const { getTranscriptionProgress } = await import("../lib/transcription");
    vi.mocked(getTranscriptionProgress).mockResolvedValue(null);
  });

  it("initialise en phase idle avec les périphériques audio", async () => {
    const { result } = renderHook(() => useMeetingFlow());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.flowPhase).toBe("idle");
    expect(result.current.canStartRecording).toBe(true);
    expect(result.current.devices).toHaveLength(1);
  });

  it("passe en phase ready après import MP3", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "import_mp3_meeting") {
        return Promise.resolve({
          id: "meeting-1",
          title: "Comité produit",
          description: null,
          status: "processing",
          startedAt: null,
          endedAt: null,
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
          audioFiles: [
            {
              id: "audio-1",
              meetingId: "meeting-1",
              filePath: "/tmp/import.mp3",
              durationMs: 120_000,
              format: "mp3",
              createdAt: "2026-01-01T00:00:00Z",
            },
          ],
          transcriptions: [],
          summaries: [],
          actions: [],
        });
      }
      if (command === "list_audio_input_devices") {
        return Promise.resolve([{ id: "mic-1", name: "Micro intégré", isDefault: true }]);
      }
      if (command === "get_selected_audio_input_device") {
        return Promise.resolve({ id: "mic-1", name: "Micro intégré", isDefault: true });
      }
      if (command === "get_recording_status") {
        return Promise.resolve({
          phase: "idle",
          deviceId: "mic-1",
          filePath: null,
          durationSecs: null,
          error: null,
        });
      }
      return Promise.resolve(null);
    });

    const { open } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(open).mockResolvedValue("/tmp/import.mp3");

    const { result } = renderHook(() => useMeetingFlow());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      await result.current.handlePickMp3();
    });

    await waitFor(() => {
      expect(result.current.flowPhase).toBe("ready");
    });
    expect(result.current.title).toBe("Comité produit");
    expect(result.current.filePath).toBe("/tmp/import.mp3");
  });

  it("passe en phase recording après démarrage", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "start_microphone_recording") {
        return Promise.resolve({
          phase: "recording",
          deviceId: "mic-1",
          filePath: null,
          durationSecs: 0,
          error: null,
        });
      }
      if (command === "list_audio_input_devices") {
        return Promise.resolve([{ id: "mic-1", name: "Micro intégré", isDefault: true }]);
      }
      if (command === "get_selected_audio_input_device") {
        return Promise.resolve({ id: "mic-1", name: "Micro intégré", isDefault: true });
      }
      if (command === "get_recording_status") {
        return Promise.resolve({
          phase: "idle",
          deviceId: "mic-1",
          filePath: null,
          durationSecs: null,
          error: null,
        });
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useMeetingFlow());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      await result.current.handleStartRecording();
    });

    expect(result.current.flowPhase).toBe("recording");
    expect(result.current.isRecording).toBe(true);
  });

  it("reprend la phase recording depuis le statut natif", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_audio_input_devices") {
        return Promise.resolve([{ id: "mic-1", name: "Micro intégré", isDefault: true }]);
      }
      if (command === "get_selected_audio_input_device") {
        return Promise.resolve({ id: "mic-1", name: "Micro intégré", isDefault: true });
      }
      if (command === "get_recording_status") {
        return Promise.resolve({
          phase: "recording",
          deviceId: "mic-1",
          filePath: null,
          durationSecs: 18,
          error: null,
        });
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useMeetingFlow());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.flowPhase).toBe("recording");
    expect(result.current.isRecording).toBe(true);
    expect(result.current.recordingStatus?.durationSecs).toBe(18);
  });

  it("reprend un traitement IA en cours sans relancer de job", async () => {
    const { getTranscriptionProgress, transcribeAudioFile } = await import("../lib/transcription");
    vi.mocked(getTranscriptionProgress).mockResolvedValue({
      jobId: "transcription-active",
      phase: "uploading",
      message: "Envoi de l'audio…",
      meetingId: "meeting-tx",
    });

    const { result } = renderHook(() => useMeetingFlow());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.flowPhase).toBe("processing");
    expect(result.current.processingStep).toBe("transcribing");
    expect(result.current.meetingId).toBe("meeting-tx");
    expect(transcribeAudioFile).not.toHaveBeenCalled();
  });

  it("ne lance qu'un traitement facturable sur double clic", async () => {
    const { transcribeAudioFile } = await import("../lib/transcription");
    const { generateStructuredSummary } = await import("../lib/ai/api");
    vi.mocked(transcribeAudioFile).mockImplementation(async (input) => ({
      jobId: input.jobId ?? "transcription-test",
      transcription: {
        id: "tx-1",
        meetingId: "meeting-1",
        content: "Transcription",
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
      },
    }));
    vi.mocked(generateStructuredSummary).mockImplementation(async (input) => ({
      jobId: input.jobId ?? "summary-test",
      meetingId: input.meetingId ?? "meeting-1",
      summary: {
        id: "summary-1",
        meetingId: input.meetingId ?? "meeting-1",
        providerId: "mistral",
        content: "Résumé",
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
      },
      structured: {
        synthese: "Résumé",
        decisions: [],
        actions: [],
        risques: [],
        questionsOuvertes: [],
      },
      actions: [],
    }));

    const { result } = renderHook(() => useMeetingFlow());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      await Promise.all([
        result.current.runProcessing({ filePath: "/tmp/import.mp3", meetingTitle: "Comité" }),
        result.current.runProcessing({ filePath: "/tmp/import.mp3", meetingTitle: "Comité" }),
      ]);
    });

    expect(transcribeAudioFile).toHaveBeenCalledTimes(1);
    expect(generateStructuredSummary).toHaveBeenCalledTimes(1);
    expect(vi.mocked(transcribeAudioFile).mock.calls[0][0].jobId).toMatch(/^transcription-/);
    expect(vi.mocked(generateStructuredSummary).mock.calls[0][0].jobId).toMatch(/^summary-/);
  });
});
