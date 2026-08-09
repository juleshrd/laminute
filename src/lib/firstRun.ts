import { invoke } from "@tauri-apps/api/core";

import type { AudioInputDevice } from "./audio";
import { getKeepAudioFiles, setKeepAudioFiles } from "./audioSettings";
import { getLocalStorageInfo, type LocalStorageInfo } from "./privacy";

export interface FirstRunStatus {
  storage: LocalStorageInfo;
  keepAudioFiles: boolean;
  selectedDevice: AudioInputDevice | null;
  deviceCount: number;
}

async function ensureAudioDevice(): Promise<{
  selectedDevice: AudioInputDevice | null;
  deviceCount: number;
}> {
  try {
    const devices = await invoke<AudioInputDevice[]>("list_audio_input_devices");
    const selectedDevice = await invoke<AudioInputDevice | null>(
      "ensure_default_audio_input_device",
    );
    return { selectedDevice, deviceCount: devices.length };
  } catch {
    // L'absence de périphérique (ou une permission micro refusée) ne bloque pas
    // le premier lancement : l'import MP3 reste pleinement disponible.
    return { selectedDevice: null, deviceCount: 0 };
  }
}

/**
 * Prépare les éléments locaux nécessaires au premier usage. Les chemins sont
 * calculés et créés côté Rust ; le WebView ne peut pas fournir de destination.
 */
export async function prepareFirstRun(): Promise<FirstRunStatus> {
  const [storage, keepAudioFiles, audio] = await Promise.all([
    getLocalStorageInfo(),
    getKeepAudioFiles(),
    ensureAudioDevice(),
  ]);

  return {
    storage,
    keepAudioFiles,
    selectedDevice: audio.selectedDevice,
    deviceCount: audio.deviceCount,
  };
}

export async function saveFirstRunStoragePreference(keepAudioFiles: boolean): Promise<boolean> {
  return setKeepAudioFiles(keepAudioFiles);
}
