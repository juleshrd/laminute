import { invoke } from "@tauri-apps/api/core";

export interface DiagnosticEvent {
  code: string;
  message: string;
  correlationId?: string | null;
  timestamp: string;
  subsystem: string;
}

export interface DiagnosticsSnapshot {
  appVersion: string;
  os: string;
  arch: string;
  appDataDir: string;
  logsDir: string;
  dbPath: string;
  dbSchemaVersion?: number | null;
  providerId?: string | null;
  transcriptionModel?: string | null;
  summaryModel?: string | null;
  keyringStatus: string;
  microphoneStatus: string;
  updaterStatus: string;
  recentErrors: DiagnosticEvent[];
}

export interface SupportBundleFilePreview {
  name: string;
  sizeBytes: number;
  textPreview?: string | null;
}

export interface SupportBundlePreview {
  files: SupportBundleFilePreview[];
  previewText: string;
  githubReport: string;
}

export function getDiagnosticsSnapshot(): Promise<DiagnosticsSnapshot> {
  return invoke<DiagnosticsSnapshot>("get_diagnostics_snapshot");
}

export function previewSupportBundle(): Promise<SupportBundlePreview> {
  return invoke<SupportBundlePreview>("preview_support_bundle");
}

export function saveSupportBundle(): Promise<boolean> {
  return invoke<boolean>("save_support_bundle");
}

export function reportDiagnosticEvent(input: {
  code: string;
  message: string;
  subsystem?: string;
  correlationId?: string;
}): Promise<void> {
  return invoke<void>("report_diagnostic_event", { input });
}

/** Capture une erreur Tauri / JS récupérable sans contenu sensible. */
export async function captureRecoverableError(
  error: unknown,
  subsystem: string,
  code = "frontend_error",
): Promise<void> {
  let message: string;
  if (typeof error === "object" && error !== null && "message" in error) {
    message = String((error as { message: unknown }).message);
  } else if (error instanceof Error) {
    message = error.message;
  } else {
    message = String(error);
  }
  // Tronquer pour éviter d'envoyer des corps collés par erreur.
  const clipped = message.slice(0, 400);
  try {
    await reportDiagnosticEvent({ code, message: clipped, subsystem });
  } catch {
    // best-effort : ne pas masquer l'erreur d'origine
  }
}
