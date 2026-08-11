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
  /** `auto` ou code langue (`fr`, `en`, …). */
  transcriptionLanguage?: string;
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
  transcriptionLanguage?: string | null;
}

export type ItemOrigin = "generated" | "edited" | "validated" | "locked";
export type SummaryValidationState = "generated" | "edited" | "validated";

export interface EvidenceSource {
  segmentIndex?: number;
  startMs?: number;
  endMs?: number;
  quote?: string;
}

export interface StructuredDecisionItem {
  texte: string;
  id?: string;
  sources?: EvidenceSource[];
  origin?: ItemOrigin;
}

export type DecisionEntry = string | StructuredDecisionItem;

export interface StructuredActionItem {
  titre: string;
  description?: string;
  responsable?: string;
  echeance?: string;
  id?: string;
  sources?: EvidenceSource[];
  origin?: ItemOrigin;
}

export interface StructuredSummary {
  synthese: string;
  decisions: DecisionEntry[];
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
  itemKey?: string;
  sources?: EvidenceSource[];
  origin?: ItemOrigin;
  createdAt: string;
  updatedAt: string;
}

export interface SummaryRecord {
  id: string;
  meetingId: string;
  providerId?: string;
  content: string;
  model?: string;
  validationState?: SummaryValidationState;
  validatedAt?: string;
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
