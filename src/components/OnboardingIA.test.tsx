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

vi.mock("../lib/ai/api", () => ({
  listAiProviders: (...args: unknown[]) => listAiProviders(...args),
  getAiSettings: (...args: unknown[]) => getAiSettings(...args),
  setSelectedProvider: (...args: unknown[]) => setSelectedProvider(...args),
  saveApiKey: (...args: unknown[]) => saveApiKey(...args),
  validateApiKey: (...args: unknown[]) => validateApiKey(...args),
  setOllamaBaseUrl: (...args: unknown[]) => setOllamaBaseUrl(...args),
  deleteApiKey: (...args: unknown[]) => deleteApiKey(...args),
}));

const PROVIDERS = [
  {
    id: "mistral",
    displayName: "Mistral AI",
    capabilities: { transcription: true, summary: true, local: false, streaming: true },
  },
  {
    id: "openai",
    displayName: "OpenAI",
    capabilities: { transcription: true, summary: true, local: false, streaming: false },
  },
  {
    id: "ollama",
    displayName: "Ollama",
    capabilities: { transcription: false, summary: true, local: true, streaming: false },
  },
];

describe("OnboardingIA", () => {
  beforeEach(() => {
    listAiProviders.mockResolvedValue(PROVIDERS);
    getAiSettings.mockResolvedValue({
      selectedProviderId: "mistral",
      hasApiKey: false,
      ollamaBaseUrl: "http://127.0.0.1:11434",
    });
    setSelectedProvider.mockImplementation(async (id: string) => ({
      selectedProviderId: id,
      hasApiKey: false,
      ollamaBaseUrl: "http://127.0.0.1:11434",
    }));
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("affiche l'accueil au premier rendu", async () => {
    render(<OnboardingIA onComplete={vi.fn()} onSkip={vi.fn()} />);

    expect(await screen.findByRole("heading", { name: "Bienvenue" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Commencer" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Plus tard" })).toBeInTheDocument();
  });

  it("met Mistral en avant à l'étape de choix", async () => {
    render(<OnboardingIA onComplete={vi.fn()} onSkip={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: "Commencer" }));

    expect(await screen.findByRole("heading", { name: "Choisir l'IA" })).toBeInTheDocument();
    expect(screen.getByText("Recommandé")).toBeInTheDocument();

    const mistral = screen.getByRole("button", { name: /Mistral AI/i });
    expect(mistral).toHaveClass("is-featured");
    expect(mistral).toHaveAttribute("aria-pressed", "true");
  });

  it("permet d'entrer dans l'app sans valider de clé", async () => {
    const onComplete = vi.fn();
    render(<OnboardingIA onComplete={onComplete} onSkip={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: "Commencer" }));
    fireEvent.click(await screen.findByRole("button", { name: "Continuer" }));

    expect(
      await screen.findByRole("heading", { name: /Configurer Mistral AI/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(/Optionnel pour l’instant/i)).toBeInTheDocument();
    expect(screen.getByLabelText("Clé API")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Entrer dans l'app" }));

    await waitFor(() => {
      expect(setSelectedProvider).toHaveBeenCalledWith("mistral");
      expect(onComplete).toHaveBeenCalledOnce();
    });
  });

  it("appelle onSkip via Plus tard depuis l'accueil", async () => {
    const onSkip = vi.fn();
    render(<OnboardingIA onComplete={vi.fn()} onSkip={onSkip} />);

    fireEvent.click(await screen.findByRole("button", { name: "Plus tard" }));
    expect(onSkip).toHaveBeenCalledOnce();
  });

  it("affiche le formulaire Ollama après sélection locale", async () => {
    render(<OnboardingIA onComplete={vi.fn()} onSkip={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: "Commencer" }));
    fireEvent.click(await screen.findByRole("button", { name: /Ollama/i }));
    fireEvent.click(screen.getByRole("button", { name: "Continuer" }));

    expect(await screen.findByLabelText("URL du serveur Ollama")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Tester la connexion" })).toBeInTheDocument();
  });
});
