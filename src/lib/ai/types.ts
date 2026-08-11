export interface ProviderCapabilities {
  transcription: boolean;
  summary: boolean;
  local: boolean;
  streaming: boolean;
  diarization: boolean;
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
  ollamaAllowRemote: boolean;
  diarizationEnabled: boolean;
  transcriptionModel?: string;
  summaryModel?: string;
  transcriptionModels: ModelInfo[];
  summaryModels: ModelInfo[];
}

export interface SetModelPreferencesInput {
  providerId: string;
  transcriptionModel?: string | null;
  summaryModel?: string | null;
  diarizationEnabled?: boolean | null;
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
  jobId?: string;
  meetingId?: string;
  text?: string;
  providerId?: string;
  model?: string;
}

export interface SummaryPipelineMeta {
  pipelineUsed?: boolean;
  estimatedInputTokens?: number;
  chunkCount?: number;
  estimatedCostUsd?: number;
}

export interface GenerateStructuredSummaryOutput {
  jobId: string;
  meetingId: string;
  summary: SummaryRecord;
  structured: StructuredSummary;
  actions: Action[];
  meta?: SummaryPipelineMeta;
}
