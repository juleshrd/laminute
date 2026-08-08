import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

import App from "./App";
import { setOnboardingDone } from "./lib/preferences";

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

vi.mock("./lib/updater", () => ({
  checkForAppUpdate: vi.fn().mockResolvedValue(null),
  probeAppUpdate: vi.fn().mockResolvedValue({ status: "up-to-date" }),
  applyAppUpdate: vi.fn(),
}));

vi.mock("./lib/ai/api", () => ({
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

vi.mock("./lib/transcription", async () => {
  const actual = await vi.importActual<typeof import("./lib/transcription")>("./lib/transcription");
  return {
    ...actual,
    getTranscriptionProgress: vi.fn().mockResolvedValue(null),
    listenTranscriptionProgress: vi.fn().mockResolvedValue(() => undefined),
    transcribeAudioFile: vi.fn(),
  };
});

vi.mock("./components/MeetingHistory", () => ({
  MeetingHistory: () => <div>Historique des réunions</div>,
}));

vi.mock("./components/SettingsScreen", () => ({
  SettingsScreen: () => <div>Réglages</div>,
}));

function setupInvoke(options?: { recording?: boolean; durationSecs?: number }) {
  let recording = options?.recording ?? false;
  let durationSecs = options?.durationSecs ?? 0;

  invokeMock.mockImplementation((command: string) => {
    switch (command) {
      case "list_audio_input_devices":
        return Promise.resolve([{ id: "mic-1", name: "Micro intégré", isDefault: true }]);
      case "get_selected_audio_input_device":
        return Promise.resolve({ id: "mic-1", name: "Micro intégré", isDefault: true });
      case "start_microphone_recording":
        recording = true;
        durationSecs = 12;
        return Promise.resolve({
          phase: "recording",
          deviceId: "mic-1",
          filePath: null,
          durationSecs,
          error: null,
        });
      case "stop_microphone_recording":
        recording = false;
        return Promise.resolve({
          phase: "stopped",
          deviceId: "mic-1",
          filePath: "/tmp/rec.wav",
          durationSecs,
          error: null,
        });
      case "get_recording_status":
        return Promise.resolve({
          phase: recording ? "recording" : "idle",
          deviceId: "mic-1",
          filePath: null,
          durationSecs: recording ? durationSecs : null,
          error: null,
        });
      default:
        return Promise.resolve(null);
    }
  });
}

async function startRecordingFromUi() {
  await screen.findByRole("button", { name: "Démarrer l'enregistrement" });
  fireEvent.click(screen.getByRole("button", { name: "Démarrer l'enregistrement" }));
  fireEvent.click(
    await screen.findByRole("button", {
      name: /J'ai informé les participants — Démarrer/i,
    }),
  );
  expect(await screen.findByRole("button", { name: "Terminer la réunion" })).toBeInTheDocument();
}

describe("App navigation + enregistrement", () => {
  beforeEach(async () => {
    invokeMock.mockReset();
    listenMock.mockClear();
    setOnboardingDone(true);
    setupInvoke();
    const { getTranscriptionProgress, transcribeAudioFile } = await import("./lib/transcription");
    vi.mocked(getTranscriptionProgress).mockResolvedValue(null);
    vi.mocked(transcribeAudioFile).mockReset();
    vi.mocked(transcribeAudioFile).mockImplementation(async (input) => ({
      jobId: input.jobId ?? "transcription-test",
      transcription: {
        id: "tx-1",
        meetingId: "meeting-1",
        content: "ok",
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
      },
    }));
  });

  afterEach(() => {
    cleanup();
  });

  it("conserve chrono, état et arrêt après navigation aller-retour", async () => {
    render(<App />);

    await startRecordingFromUi();
    expect(screen.getByLabelText("Durée de l'enregistrement")).toHaveTextContent("0:12");
    expect(screen.getByRole("button", { name: /Micro actif/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Historique" }));
    expect(screen.getByText("Historique des réunions")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Terminer la réunion" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Micro actif/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Réunion courante" }));
    expect(await screen.findByRole("button", { name: "Terminer la réunion" })).toBeInTheDocument();
    expect(screen.getByLabelText("Durée de l'enregistrement")).toHaveTextContent("0:12");
  });

  it("arrête effectivement l'enregistrement après un aller-retour", async () => {
    render(<App />);

    await startRecordingFromUi();
    fireEvent.click(screen.getByRole("button", { name: "Historique" }));
    fireEvent.click(screen.getByRole("button", { name: /Micro actif/i }));

    expect(await screen.findByRole("button", { name: "Terminer la réunion" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Terminer la réunion" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("stop_microphone_recording");
    });
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: /Micro actif/i })).not.toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: "Terminer la réunion" })).not.toBeInTheDocument();
  });

  it("restaure un traitement IA actif sans second job après navigation", async () => {
    const { getTranscriptionProgress, transcribeAudioFile } = await import("./lib/transcription");
    vi.mocked(getTranscriptionProgress).mockResolvedValue({
      jobId: "transcription-nav",
      phase: "transcribing",
      message: "Transcription en cours…",
      meetingId: "meeting-nav",
    });

    render(<App />);

    expect(await screen.findByText(/Transcription en cours/i)).toBeInTheDocument();
    vi.mocked(transcribeAudioFile).mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Historique" }));
    fireEvent.click(screen.getByRole("button", { name: "Réunion courante" }));

    expect(await screen.findByText(/Transcription en cours/i)).toBeInTheDocument();
    expect(transcribeAudioFile).not.toHaveBeenCalled();
  });
});
