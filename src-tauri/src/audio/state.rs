use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::devices::{list_input_devices, AudioInputDevice};
use super::error::AudioError;
use super::recording::{RecordingPhase, RecordingService, RecordingStatus};

#[derive(Debug, Serialize, Deserialize)]
struct PersistedSettings {
    selected_device_id: Option<String>,
    #[serde(default = "default_keep_audio_files")]
    keep_audio_files: bool,
}

fn default_keep_audio_files() -> bool {
    true
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            selected_device_id: None,
            keep_audio_files: true,
        }
    }
}

pub struct AudioState {
    selected_device_id: Mutex<Option<String>>,
    keep_audio_files: Mutex<bool>,
    recording: RecordingService,
    settings_path: PathBuf,
    recordings_dir: PathBuf,
}

impl AudioState {
    pub fn initialize(app: &AppHandle) -> Result<Self, AudioError> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|err| AudioError::Io(err.to_string()))?;

        let settings_path = app_data_dir.join("audio-settings.json");
        let recordings_dir = app_data_dir.join("recordings");

        let persisted = load_settings(&settings_path);
        let mut selected_device_id = persisted.selected_device_id;
        let keep_audio_files = persisted.keep_audio_files;

        if let Some(device_id) = &selected_device_id {
            if resolve_selected_device(device_id).is_err() {
                selected_device_id = None;
                let _ = save_settings(
                    &settings_path,
                    &PersistedSettings {
                        selected_device_id: None,
                        keep_audio_files,
                    },
                );
            }
        }

        Ok(Self {
            selected_device_id: Mutex::new(selected_device_id),
            keep_audio_files: Mutex::new(keep_audio_files),
            recording: RecordingService::spawn(),
            settings_path,
            recordings_dir,
        })
    }

    pub fn list_devices(&self) -> Result<Vec<AudioInputDevice>, AudioError> {
        list_input_devices()
    }

    pub fn get_selected_device(&self) -> Result<Option<AudioInputDevice>, AudioError> {
        let selected_id = self
            .selected_device_id
            .lock()
            .map_err(|_| AudioError::Internal("verrou état audio indisponible".into()))?
            .clone();

        let Some(device_id) = selected_id else {
            return Ok(None);
        };

        let devices = list_input_devices()?;
        Ok(devices.into_iter().find(|device| device.id == device_id))
    }

    pub fn set_selected_device(&self, device_id: String) -> Result<AudioInputDevice, AudioError> {
        let device = resolve_selected_device(&device_id)?;

        {
            let mut selected = self
                .selected_device_id
                .lock()
                .map_err(|_| AudioError::Internal("verrou état audio indisponible".into()))?;
            *selected = Some(device_id);
        }

        self.persist_settings()?;

        Ok(device)
    }

    pub fn keep_audio_files(&self) -> Result<bool, AudioError> {
        self.keep_audio_files
            .lock()
            .map(|value| *value)
            .map_err(|_| AudioError::Internal("verrou état audio indisponible".into()))
    }

    pub fn set_keep_audio_files(&self, keep: bool) -> Result<bool, AudioError> {
        {
            let mut value = self
                .keep_audio_files
                .lock()
                .map_err(|_| AudioError::Internal("verrou état audio indisponible".into()))?;
            *value = keep;
        }
        self.persist_settings()?;
        Ok(keep)
    }

    fn persist_settings(&self) -> Result<(), AudioError> {
        let selected_device_id = self
            .selected_device_id
            .lock()
            .map_err(|_| AudioError::Internal("verrou état audio indisponible".into()))?
            .clone();
        let keep_audio_files = self
            .keep_audio_files
            .lock()
            .map_err(|_| AudioError::Internal("verrou état audio indisponible".into()))
            .map(|value| *value)?;

        save_settings(
            &self.settings_path,
            &PersistedSettings {
                selected_device_id,
                keep_audio_files,
            },
        )
    }

    pub fn recording_status(&self) -> Result<RecordingStatus, AudioError> {
        self.recording.status()
    }

    pub fn start_recording(&self) -> Result<RecordingStatus, AudioError> {
        let device_id = self
            .selected_device_id
            .lock()
            .map_err(|_| AudioError::Internal("verrou état audio indisponible".into()))?
            .clone()
            .ok_or(AudioError::DeviceNotFound(
                "aucun périphérique sélectionné".into(),
            ))?;

        self.recording.start(&device_id, &self.recordings_dir)
    }

    pub fn stop_recording(&self) -> Result<RecordingStatus, AudioError> {
        self.recording.stop()
    }

    /// Arrête l'enregistrement s'il est actif (no-op si idle).
    pub fn stop_recording_if_active(&self) -> Result<(), AudioError> {
        if self.recording.status()?.phase == RecordingPhase::Recording {
            self.recording.stop()?;
        }
        Ok(())
    }

    /// Remet les réglages audio mémoire/disque aux valeurs par défaut.
    pub fn reset_persisted_settings(&self) -> Result<(), AudioError> {
        {
            let mut selected = self
                .selected_device_id
                .lock()
                .map_err(|_| AudioError::Internal("verrou état audio indisponible".into()))?;
            *selected = None;
        }
        {
            let mut keep = self
                .keep_audio_files
                .lock()
                .map_err(|_| AudioError::Internal("verrou état audio indisponible".into()))?;
            *keep = default_keep_audio_files();
        }

        match fs::remove_file(&self.settings_path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(AudioError::Io(err.to_string())),
        }
    }
}

fn resolve_selected_device(device_id: &str) -> Result<AudioInputDevice, AudioError> {
    let devices = list_input_devices()?;
    devices
        .into_iter()
        .find(|device| device.id == device_id)
        .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))
}

fn load_settings(path: &PathBuf) -> PersistedSettings {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save_settings(path: &PathBuf, settings: &PersistedSettings) -> Result<(), AudioError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_string_pretty(settings)?;
    fs::write(path, payload)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_settings_roundtrip() {
        let dir =
            std::env::temp_dir().join(format!("laminute-audio-settings-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("audio-settings.json");

        let settings = PersistedSettings {
            selected_device_id: Some("input-0".into()),
            keep_audio_files: false,
        };
        save_settings(&path, &settings).expect("save settings");

        let loaded = load_settings(&path);
        assert_eq!(loaded.selected_device_id, Some("input-0".into()));
        assert!(!loaded.keep_audio_files);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn keep_audio_files_defaults_to_true() {
        let loaded: PersistedSettings = serde_json::from_str("{}").expect("empty defaults");
        assert!(loaded.keep_audio_files);
    }
}
