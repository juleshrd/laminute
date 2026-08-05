import { describe, expect, it } from "vitest";

import type { ProviderInfo } from "./types";

export function formatCapabilities(provider: ProviderInfo): string {
  const labels: Record<string, string> = {
    transcription: "Transcription",
    summary: "Résumé",
    local: "Local",
    streaming: "Streaming",
  };

  return Object.entries(provider.capabilities)
    .filter(([, enabled]) => enabled)
    .map(([key]) => labels[key] ?? key)
    .join(" · ");
}

describe("formatCapabilities", () => {
  it("affiche uniquement les capacités activées", () => {
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
});
