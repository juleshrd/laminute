export interface ProviderCapabilities {
  transcription: boolean;
  summary: boolean;
  local: boolean;
  streaming: boolean;
}

export interface ProviderInfo {
  id: string;
  displayName: string;
  capabilities: ProviderCapabilities;
}

export interface ModelInfo {
  id: string;
  name: string;
  description?: string;
}

export interface KeyValidationResult {
  valid: boolean;
  message: string;
  models?: ModelInfo[];
}

export interface AiSettings {
  selectedProviderId?: string;
  hasApiKey: boolean;
}
