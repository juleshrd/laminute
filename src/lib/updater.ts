import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateProgress = {
  downloaded: number;
  contentLength: number | null;
  finished: boolean;
};

export type UpdateProgressHandler = (progress: UpdateProgress) => void;

/** Vérifie s'il existe une mise à jour. Renvoie `null` si aucune ou en cas d'échec silencieux côté appelant. */
export async function checkForAppUpdate(): Promise<Update | null> {
  return check();
}

/** Télécharge, installe la mise à jour puis relance l'application. */
export async function applyAppUpdate(
  update: Update,
  onProgress?: UpdateProgressHandler,
): Promise<void> {
  let downloaded = 0;
  let contentLength: number | null = null;

  const report = (finished: boolean) => {
    onProgress?.({ downloaded, contentLength, finished });
  };

  await update.downloadAndInstall((event: DownloadEvent) => {
    switch (event.event) {
      case "Started":
        contentLength = event.data.contentLength ?? null;
        downloaded = 0;
        report(false);
        break;
      case "Progress":
        downloaded += event.data.chunkLength;
        report(false);
        break;
      case "Finished":
        report(true);
        break;
    }
  });

  await relaunch();
}

export function formatUpdateProgress(progress: UpdateProgress): string {
  if (progress.finished) {
    return "Installation en cours…";
  }
  if (progress.contentLength && progress.contentLength > 0) {
    const pct = Math.min(100, Math.round((progress.downloaded / progress.contentLength) * 100));
    return `Téléchargement… ${pct} %`;
  }
  if (progress.downloaded > 0) {
    return `Téléchargement… ${Math.round(progress.downloaded / 1024)} Ko`;
  }
  return "Téléchargement…";
}
