import { invoke } from "@tauri-apps/api/core";

export interface LocalStorageInfo {
  meetingsCount: number;
  dbPath: string;
  importsDir: string;
  recordingsDir: string;
  importsBytes?: number | null;
  recordingsBytes?: number | null;
}

export type MeetingExportFormat = "json" | "markdown" | "pdf";

/** Dialogue natif + écriture côté Rust. Retourne false si l'utilisateur annule. */
export function saveMeetingExport(
  id: string,
  format: MeetingExportFormat,
  defaultFileName: string,
): Promise<boolean> {
  return invoke<boolean>("save_meeting_export", { id, format, defaultFileName });
}

export function getLocalStorageInfo(): Promise<LocalStorageInfo> {
  return invoke<LocalStorageInfo>("get_local_storage_info");
}

export function deleteAllLocalData(): Promise<void> {
  return invoke<void>("delete_all_local_data");
}

export function buildExportFilename(
  title: string,
  exportedAt: string,
  extension: string = "json",
): string {
  const safe =
    title
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .replace(/[^\w\s-]/g, "")
      .trim()
      .replace(/\s+/g, "-")
      .slice(0, 50) || "reunion";
  const date = exportedAt.slice(0, 10);
  const ext = extension.replace(/^\./, "") || "json";
  return `laminute-${safe}-${date}.${ext}`;
}

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || Number.isNaN(bytes)) {
    return "—";
  }
  if (bytes < 1024) {
    return `${bytes} o`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} Ko`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} Mo`;
}
