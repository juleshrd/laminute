import { invoke } from "@tauri-apps/api/core";

import type {
  AiSettings,
  GenerateStructuredSummaryInput,
  GenerateStructuredSummaryOutput,
  KeyValidationResult,
  ProviderInfo,
  SetModelPreferencesInput,
} from "./types";

export function listAiProviders(): Promise<ProviderInfo[]> {
  return invoke<ProviderInfo[]>("list_ai_providers");
}

export function getAiSettings(): Promise<AiSettings> {
  return invoke<AiSettings>("get_ai_settings");
}

export function setSelectedProvider(providerId: string): Promise<AiSettings> {
  return invoke<AiSettings>("set_selected_provider", { providerId });
}

export function setOllamaBaseUrl(baseUrl: string): Promise<AiSettings> {
  return invoke<AiSettings>("set_ollama_base_url", { baseUrl });
}

export function setModelPreferences(input: SetModelPreferencesInput): Promise<AiSettings> {
  return invoke<AiSettings>("set_model_preferences", { input });
}

export function saveApiKey(providerId: string, apiKey: string): Promise<void> {
  return invoke<void>("save_api_key", { providerId, apiKey });
}

export function deleteApiKey(providerId: string): Promise<void> {
  return invoke<void>("delete_api_key", { providerId });
}

export function validateApiKey(providerId: string, apiKey?: string): Promise<KeyValidationResult> {
  return invoke<KeyValidationResult>("validate_api_key", {
    providerId,
    apiKey: apiKey ?? null,
  });
}

export function generateStructuredSummary(
  input: GenerateStructuredSummaryInput,
): Promise<GenerateStructuredSummaryOutput> {
  return invoke<GenerateStructuredSummaryOutput>("generate_structured_summary", {
    input,
  });
}
