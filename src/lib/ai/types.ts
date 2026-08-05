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
  ollamaBaseUrl?: string;
}

export interface StructuredActionItem {
  titre: string;
  description?: string;
  responsable?: string;
  echeance?: string;
}

export interface StructuredSummary {
  synthese: string;
  decisions: string[];
  actions: StructuredActionItem[];
  risques: string[];
  questionsOuvertes: string[];
}

export interface Action {
  id: string;
  meetingId: string;
  title: string;
  description?: string;
  assignee?: string;
  dueDate?: string;
  status: string;
  createdAt: string;
  updatedAt: string;
}

export interface SummaryRecord {
  id: string;
  meetingId: string;
  providerId?: string;
  content: string;
  createdAt: string;
  updatedAt: string;
}

export interface GenerateStructuredSummaryInput {
  meetingId?: string;
  text?: string;
  providerId?: string;
  model?: string;
}

export interface GenerateStructuredSummaryOutput {
  meetingId: string;
  summary: SummaryRecord;
  structured: StructuredSummary;
  actions: Action[];
}
