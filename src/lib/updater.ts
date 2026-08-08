import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateProgress = {
  downloaded: number;
  contentLength: number | null;
  finished: boolean;
};

export type UpdateProgressHandler = (progress: UpdateProgress) => void;

export type UpdateCheckResult =
  | { status: "available"; update: Update }
  | { status: "up-to-date" }
  | { status: "error"; message: string };

/** Message utilisateur pour une erreur de vérification de mise à jour. */
export function describeUpdateCheckError(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return `Vérification des mises à jour impossible : ${error.message}`;
  }
  return "Vérification des mises à jour impossible (réseau indisponible ou flux inaccessible).";
}

/** Vérifie s'il existe une mise à jour. Propague les erreurs réseau au lieu de les avaler. */
export async function checkForAppUpdate(): Promise<Update | null> {
  return check();
}

/** Résultat discriminant : update, à jour, ou erreur diagnosticable. */
export async function probeAppUpdate(): Promise<UpdateCheckResult> {
  try {
    const update = await checkForAppUpdate();
    if (update) {
      return { status: "available", update };
    }
    return { status: "up-to-date" };
  } catch (error) {
    return { status: "error", message: describeUpdateCheckError(error) };
  }
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
