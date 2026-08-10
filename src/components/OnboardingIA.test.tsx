import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

import { OnboardingIA } from "./OnboardingIA";

const listAiProviders = vi.fn();
const getAiSettings = vi.fn();
const setSelectedProvider = vi.fn();
const saveApiKey = vi.fn();
const validateApiKey = vi.fn();
const setOllamaBaseUrl = vi.fn();
const deleteApiKey = vi.fn();
const prepareFirstRun = vi.fn();
const saveFirstRunStoragePreference = vi.fn();

vi.mock("../lib/ai/api", () => ({
  listAiProviders: (...args: unknown[]) => listAiProviders(...args),
  getAiSettings: (...args: unknown[]) => getAiSettings(...args),
  setSelectedProvider: (...args: unknown[]) => setSelectedProvider(...args),
  saveApiKey: (...args: unknown[]) => saveApiKey(...args),
  validateApiKey: (...args: unknown[]) => validateApiKey(...args),
  setOllamaBaseUrl: (...args: unknown[]) => setOllamaBaseUrl(...args),
  deleteApiKey: (...args: unknown[]) => deleteApiKey(...args),
}));

vi.mock("../lib/firstRun", () => ({
  prepareFirstRun: (...args: unknown[]) => prepareFirstRun(...args),
  saveFirstRunStoragePreference: (...args: unknown[]) => saveFirstRunStoragePreference(...args),
}));

const PROVIDERS = [
  {
    id: "mistral",
    displayName: "Mistral AI",
    capabilities: {
      transcription: true,
      summary: true,
      local: false,
      streaming: true,
      diarization: true,
    },
  },
  {
    id: "openai",
    displayName: "OpenAI",
    capabilities: {
      transcription: true,
      summary: true,
      local: false,
      streaming: false,
      diarization: true,
    },
  },
  {
    id: "ollama",
    displayName: "Ollama",
    capabilities: {
      transcription: false,
      summary: true,
      local: true,
      streaming: false,
      diarization: false,
    },
  },
];

describe("OnboardingIA", () => {
  beforeEach(() => {
    listAiProviders.mockResolvedValue(PROVIDERS);
    getAiSettings.mockResolvedValue({
      selectedProviderId: "mistral",
      hasApiKey: false,
      ollamaBaseUrl: "http://127.0.0.1:11434",
      ollamaAllowRemote: false,
      diarizationEnabled: false,
      transcriptionModel: "voxtral-mini-latest",
      summaryModel: "mistral-small-latest",
      transcriptionModels: [],
      summaryModels: [],
    });
    setSelectedProvider.mockImplementation(async (id: string) => ({
      selectedProviderId: id,
      hasApiKey: false,
      ollamaBaseUrl: "http://127.0.0.1:11434",
      ollamaAllowRemote: false,
      diarizationEnabled: false,
      transcriptionModels: [],
      summaryModels: [],
    }));
    prepareFirstRun.mockResolvedValue({
      storage: {
        meetingsCount: 0,
        dbPath: "/Users/test/Library/Application Support/app.laminute.desktop/laminute.db",
        importsDir: "/Users/test/Library/Application Support/app.laminute.desktop/imports",
        recordingsDir: "/Users/test/Library/Application Support/app.laminute.desktop/recordings",
      },
      keepAudioFiles: true,
      selectedDevice: { id: "mic-1", name: "Micro du Mac", isDefault: true },
      deviceCount: 1,
    });
    saveFirstRunStoragePreference.mockResolvedValue(true);
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("affiche l'accueil au premier rendu", async () => {
    render(<OnboardingIA onComplete={vi.fn()} onSkip={vi.fn()} />);

    expect(await screen.findByRole("heading", { name: "Bienvenue" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Configurer l’app" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Découvrir sans configurer" })).toBeInTheDocument();
    expect(
      screen.getByText(/Les réunions restent stockées sur votre ordinateur/i),
    ).toBeInTheDocument();
    expect(screen.queryByText(/Tout reste sur votre ordinateur/i)).not.toBeInTheDocument();
    expect(
      await screen.findByRole("list", { name: "Fournisseurs IA disponibles" }),
    ).toBeInTheDocument();
    expect(document.querySelector('img[data-provider="mistral"]')).toBeTruthy();
    expect(document.querySelector('img[data-provider="openai"]')).toBeTruthy();
    expect(document.querySelector('img[data-provider="ollama"]')).toBeTruthy();
  });

  it("met Mistral en avant à l'étape de choix", async () => {
    render(<OnboardingIA onComplete={vi.fn()} onSkip={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: "Configurer l’app" }));
    expect(
      await screen.findByText(/Autorisation demandée au premier enregistrement/i),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continuer" }));

    expect(await screen.findByRole("heading", { name: "Choisir l'IA" })).toBeInTheDocument();
    expect(screen.getByText("Recommandé")).toBeInTheDocument();

    const mistral = screen.getByRole("button", { name: /Mistral AI/i });
    expect(mistral).toHaveClass("is-featured");
    expect(mistral).toHaveAttribute("aria-pressed", "true");
  });

  it("affiche le logo de chaque fournisseur à l'étape de choix", async () => {
    render(<OnboardingIA onComplete={vi.fn()} onSkip={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: "Configurer l’app" }));
    fireEvent.click(await screen.findByRole("button", { name: "Continuer" }));
    await screen.findByRole("heading", { name: "Choisir l'IA" });

    for (const id of ["mistral", "openai", "ollama"]) {
      const logo = document.querySelector(`img[data-provider="${id}"]`);
      expect(logo).toBeTruthy();
      const src = (logo as HTMLImageElement).getAttribute("src") ?? "";
      expect(src).toMatch(/(\.svg|image\/svg\+xml)/);
    }
  });

  it("rend explicite le mode limité sans clé avant d'entrer dans l'app", async () => {
    const onComplete = vi.fn();
    render(<OnboardingIA onComplete={onComplete} onSkip={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: "Configurer l’app" }));
    fireEvent.click(await screen.findByRole("button", { name: "Continuer" }));
    await screen.findByRole("heading", { name: "Choisir l'IA" });
    fireEvent.click(screen.getByRole("button", { name: "Continuer" }));

    expect(
      await screen.findByRole("heading", { name: /Configurer Mistral AI/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(/Sans clé, vous pourrez importer un MP3/i)).toBeInTheDocument();
    expect(screen.getByLabelText("Clé API")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Continuer en mode limité" }));

    expect(await screen.findByRole("heading", { name: "La Minute est prête" })).toBeInTheDocument();
    expect(onComplete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Créer ma première réunion" }));

    await waitFor(() => {
      expect(setSelectedProvider).toHaveBeenCalledWith("mistral");
      expect(onComplete).toHaveBeenCalledOnce();
    });
  });

  it("appelle onSkip sans terminer la configuration", async () => {
    const onSkip = vi.fn();
    render(<OnboardingIA onComplete={vi.fn()} onSkip={onSkip} />);

    fireEvent.click(await screen.findByRole("button", { name: "Découvrir sans configurer" }));
    expect(onSkip).toHaveBeenCalledOnce();
  });

  it("affiche le formulaire Ollama après sélection locale", async () => {
    render(<OnboardingIA onComplete={vi.fn()} onSkip={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: "Configurer l’app" }));
    fireEvent.click(await screen.findByRole("button", { name: "Continuer" }));
    fireEvent.click(await screen.findByRole("button", { name: /Ollama/i }));
    fireEvent.click(screen.getByRole("button", { name: "Continuer" }));

    expect(await screen.findByLabelText("URL du serveur Ollama")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Tester la connexion" })).toBeInTheDocument();
    expect(screen.getByText(/Serveur local \(loopback\)/i)).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("URL du serveur Ollama"), {
      target: { value: "http://192.168.1.10:11434" },
    });
    expect(screen.getByText(/Serveur distant ou LAN détecté/i)).toBeInTheDocument();
    expect(
      screen.getByLabelText(/J'autorise explicitement ce serveur Ollama/i),
    ).toBeInTheDocument();
  });

  it("bloque la suite et permet de réessayer si la préparation locale échoue", async () => {
    prepareFirstRun
      .mockRejectedValueOnce(new Error("Dossier non accessible"))
      .mockResolvedValueOnce({
        storage: {
          meetingsCount: 0,
          dbPath: "/tmp/laminute.db",
          importsDir: "/tmp/imports",
          recordingsDir: "/tmp/recordings",
        },
        keepAudioFiles: true,
        selectedDevice: null,
        deviceCount: 0,
      });

    render(<OnboardingIA onComplete={vi.fn()} onSkip={vi.fn()} />);
    fireEvent.click(await screen.findByRole("button", { name: "Configurer l’app" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Dossier non accessible");
    expect(screen.getByRole("button", { name: "Continuer" })).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "Réessayer" }));
    expect(
      await screen.findByText(/Autorisation demandée au premier enregistrement/i),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continuer" })).toBeEnabled();
  });
});
