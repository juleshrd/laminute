import type { ProviderInfo } from "./types";

const CAPABILITY_LABELS: Record<string, string> = {
  transcription: "Transcription",
  summary: "Résumé",
  local: "Local",
  streaming: "Streaming",
};

export function formatCapabilities(provider: ProviderInfo): string {
  return (Object.entries(provider.capabilities) as [string, boolean][])
    .filter(([, enabled]) => enabled)
    .map(([key]) => CAPABILITY_LABELS[key] ?? key)
    .join(" · ");
}
