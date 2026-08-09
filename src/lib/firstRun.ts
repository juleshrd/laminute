import { getKeepAudioFiles, setKeepAudioFiles } from "./audioSettings";
import { getLocalStorageInfo, type LocalStorageInfo } from "./privacy";

export interface FirstRunStatus {
  storage: LocalStorageInfo;
  keepAudioFiles: boolean;
}

/**
 * Prépare les éléments locaux nécessaires au premier usage. Les chemins sont
 * calculés et créés côté Rust ; le WebView ne peut pas fournir de destination.
 */
export async function prepareFirstRun(): Promise<FirstRunStatus> {
  const [storage, keepAudioFiles] = await Promise.all([getLocalStorageInfo(), getKeepAudioFiles()]);

  return {
    storage,
    keepAudioFiles,
  };
}

export async function saveFirstRunStoragePreference(keepAudioFiles: boolean): Promise<boolean> {
  return setKeepAudioFiles(keepAudioFiles);
}
