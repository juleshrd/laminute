import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MeetingWorkspace } from "./MeetingWorkspace";

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
  getAiSettings: vi.fn().mockResolvedValue({ hasApiKey: true, selectedProviderId: "mistral" }),
  generateStructuredSummary: vi.fn(),
}));

function setupDefaultInvoke() {
  invokeMock.mockImplementation((command: string) => {
    switch (command) {
      case "list_audio_input_devices":
        return Promise.resolve([
          { id: "mic-1", name: "Micro intégré", isDefault: true },
        ]);
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

describe("MeetingWorkspace", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockClear();
    setupDefaultInvoke();
  });

  afterEach(() => {
    cleanup();
  });

  it("affiche l'écran d'accueil avec enregistrement et import MP3", async () => {
    render(<MeetingWorkspace />);

    expect(
      await screen.findByText(/Prêt à enregistrer ou importer un fichier audio/i),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Démarrer" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Choisir un fichier MP3" })).toBeInTheDocument();
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
        return Promise.resolve([
          { id: "mic-1", name: "Micro intégré", isDefault: true },
        ]);
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

    render(<MeetingWorkspace />);
    await screen.findByRole("button", { name: "Choisir un fichier MP3" });

    fireEvent.click(screen.getByRole("button", { name: "Choisir un fichier MP3" }));

    await waitFor(() => {
      expect(screen.getByText(/Audio prêt/i)).toBeInTheDocument();
    });
    expect(screen.getByLabelText("Titre")).toHaveValue("Comité produit");
    expect(screen.getByRole("button", { name: "Traiter" })).toBeInTheDocument();
  });

  it("affiche un message si aucune clé API n'est configurée", async () => {
    const { getAiSettings } = await import("../lib/ai/api");
    vi.mocked(getAiSettings).mockResolvedValue({
      hasApiKey: false,
      selectedProviderId: "mistral",
    });

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
        return Promise.resolve([
          { id: "mic-1", name: "Micro intégré", isDefault: true },
        ]);
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

    render(<MeetingWorkspace />);
    await screen.findByRole("button", { name: "Choisir un fichier MP3" });
    fireEvent.click(screen.getByRole("button", { name: "Choisir un fichier MP3" }));

    expect(
      await screen.findByText(/Configurez une clé API Mistral dans les réglages IA/i),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Traiter" })).toBeDisabled();
  });
});
