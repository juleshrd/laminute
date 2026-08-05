use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;

use super::error::AudioError;

pub const DEVICE_ID_PREFIX: &str = "input-";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioInputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub fn device_id(index: usize) -> String {
    format!("{DEVICE_ID_PREFIX}{index}")
}

pub fn list_input_devices() -> Result<Vec<AudioInputDevice>, AudioError> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|device| device.name().ok());

    let devices: Vec<AudioInputDevice> = host
        .input_devices()
        .map_err(AudioError::from_host)?
        .enumerate()
        .filter_map(|(index, device)| {
            let name = device.name().ok()?;
            Some(AudioInputDevice {
                id: device_id(index),
                name: name.clone(),
                is_default: default_name.as_ref() == Some(&name),
            })
        })
        .collect();

    if devices.is_empty() {
        return Err(AudioError::NoInputDevice);
    }

    Ok(devices)
}

pub fn resolve_input_device(device_id: &str) -> Result<cpal::Device, AudioError> {
    if !device_id.starts_with(DEVICE_ID_PREFIX) {
        return Err(AudioError::DeviceNotFound(device_id.to_string()));
    }

    let index: usize = device_id
        .trim_start_matches(DEVICE_ID_PREFIX)
        .parse()
        .map_err(|_| AudioError::DeviceNotFound(device_id.to_string()))?;

    let host = cpal::default_host();
    host.input_devices()
        .map_err(AudioError::from_host)?
        .nth(index)
        .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_format() {
        assert_eq!(device_id(0), "input-0");
        assert_eq!(device_id(3), "input-3");
    }

    #[test]
    fn list_input_devices_returns_at_least_one_or_no_device_error() {
        match list_input_devices() {
            Ok(devices) => {
                assert!(!devices.is_empty());
                assert!(devices.iter().all(|d| d.id.starts_with(DEVICE_ID_PREFIX)));
            }
            Err(AudioError::NoInputDevice) => {}
            Err(other) => panic!("unexpected error: {other}"),
        }
    }
}
