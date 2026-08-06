import { invoke } from "@tauri-apps/api/core";

export function getKeepAudioFiles(): Promise<boolean> {
  return invoke<boolean>("get_keep_audio_files");
}

export function setKeepAudioFiles(keep: boolean): Promise<boolean> {
  return invoke<boolean>("set_keep_audio_files", { keep });
}
