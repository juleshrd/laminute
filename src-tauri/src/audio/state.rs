use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::devices::{list_input_devices, AudioInputDevice};
use super::error::AudioError;
use super::recording::{RecordingService, RecordingStatus};

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedSettings {
    selected_device_id: Option<String>,
}

pub struct AudioState {
    selected_device_id: Mutex<Option<String>>,
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

        if let Some(device_id) = &selected_device_id {
            if resolve_selected_device(device_id).is_err() {
                let _ = fs::remove_file(&settings_path);
                selected_device_id = None;
            }
        }

        Ok(Self {
            selected_device_id: Mutex::new(selected_device_id),
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

        save_settings(
            &self.settings_path,
            &PersistedSettings {
                selected_device_id: self
                    .selected_device_id
                    .lock()
                    .map_err(|_| AudioError::Internal("verrou état audio indisponible".into()))?
                    .clone(),
            },
        )?;

        Ok(device)
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
        let dir = std::env::temp_dir().join(format!(
            "laminute-audio-settings-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("audio-settings.json");

        let settings = PersistedSettings {
            selected_device_id: Some("input-0".into()),
        };
        save_settings(&path, &settings).expect("save settings");

        let loaded = load_settings(&path);
        assert_eq!(loaded.selected_device_id, Some("input-0".into()));

        let _ = fs::remove_dir_all(dir);
    }
}
