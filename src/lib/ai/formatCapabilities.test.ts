import { describe, expect, it } from "vitest";

import { formatCapabilities } from "./formatCapabilities";
import type { ProviderInfo } from "./types";

describe("formatCapabilities", () => {
  it("affiche uniquement les capacités activées pour Mistral", () => {
    const provider: ProviderInfo = {
      id: "mistral",
      displayName: "Mistral AI",
      capabilities: {
        transcription: true,
        summary: true,
        local: false,
        streaming: true,
      },
    };

    expect(formatCapabilities(provider)).toBe("Transcription · Résumé · Streaming");
  });

  it("affiche transcription et résumé pour OpenAI", () => {
    const provider: ProviderInfo = {
      id: "openai",
      displayName: "OpenAI",
      capabilities: {
        transcription: true,
        summary: true,
        local: false,
        streaming: false,
      },
    };

    expect(formatCapabilities(provider)).toBe("Transcription · Résumé");
  });

  it("affiche résumé et local pour Ollama", () => {
    const provider: ProviderInfo = {
      id: "ollama",
      displayName: "Ollama",
      capabilities: {
        transcription: false,
        summary: true,
        local: true,
        streaming: false,
      },
    };

    expect(formatCapabilities(provider)).toBe("Résumé · Local");
  });
});
