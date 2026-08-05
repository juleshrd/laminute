use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};

use super::devices::resolve_input_device;
use super::error::AudioError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingPhase {
    Idle,
    Recording,
    Stopped,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatus {
    pub phase: RecordingPhase,
    pub device_id: Option<String>,
    pub file_path: Option<String>,
    pub duration_secs: Option<f64>,
    pub error: Option<String>,
}

impl RecordingStatus {
    pub fn idle() -> Self {
        Self {
            phase: RecordingPhase::Idle,
            device_id: None,
            file_path: None,
            duration_secs: None,
            error: None,
        }
    }
}

struct ActiveRecording {
    #[allow(dead_code)]
    stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
    started_at: Instant,
    output_path: PathBuf,
    device_id: String,
}

struct RecordingController {
    active: Option<ActiveRecording>,
    last_status: RecordingStatus,
}

impl RecordingController {
    fn new() -> Self {
        Self {
            active: None,
            last_status: RecordingStatus::idle(),
        }
    }

    fn status(&self) -> RecordingStatus {
        if let Some(recording) = &self.active {
            return RecordingStatus {
                phase: RecordingPhase::Recording,
                device_id: Some(recording.device_id.clone()),
                file_path: Some(recording.output_path.to_string_lossy().into_owned()),
                duration_secs: Some(recording.started_at.elapsed().as_secs_f64()),
                error: None,
            };
        }

        self.last_status.clone()
    }

    fn start(
        &mut self,
        device_id: &str,
        recordings_dir: &Path,
    ) -> Result<RecordingStatus, AudioError> {
        if self.active.is_some() {
            return Err(AudioError::AlreadyRecording);
        }

        fs::create_dir_all(recordings_dir)?;

        let device = resolve_input_device(device_id)?;
        let config = device
            .default_input_config()
            .map_err(|err| map_config_error(err.to_string()))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let sample_format = config.sample_format();

        let stream_config = StreamConfig {
            channels,
            sample_rate: config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let err_flag = Arc::new(Mutex::new(None::<String>));

        let stream = build_input_stream(
            &device,
            &stream_config,
            sample_format,
            Arc::clone(&samples),
            err_flag,
        )?;

        stream
            .play()
            .map_err(|err| AudioError::Internal(err.to_string()))?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let output_path = recordings_dir.join(format!("recording-{timestamp}.wav"));

        self.active = Some(ActiveRecording {
            stream,
            samples,
            sample_rate,
            channels,
            started_at: Instant::now(),
            output_path,
            device_id: device_id.to_string(),
        });

        Ok(self.status())
    }

    fn stop(&mut self) -> Result<RecordingStatus, AudioError> {
        let recording = self.active.take().ok_or(AudioError::NotRecording)?;

        let duration_secs = recording.started_at.elapsed().as_secs_f64();
        let samples = recording
            .samples
            .lock()
            .map_err(|_| AudioError::Internal("verrou audio indisponible".into()))?
            .clone();

        write_wav(
            &recording.output_path,
            &samples,
            recording.sample_rate,
            recording.channels,
        )?;

        let file_path = recording.output_path.to_string_lossy().into_owned();
        self.last_status = RecordingStatus {
            phase: RecordingPhase::Stopped,
            device_id: Some(recording.device_id),
            file_path: Some(file_path),
            duration_secs: Some(duration_secs),
            error: None,
        };

        Ok(self.last_status.clone())
    }
}

enum RecordingRequest {
    Start {
        device_id: String,
        recordings_dir: PathBuf,
        reply: Sender<Result<RecordingStatus, AudioError>>,
    },
    Stop {
        reply: Sender<Result<RecordingStatus, AudioError>>,
    },
    Status {
        reply: Sender<RecordingStatus>,
    },
}

pub struct RecordingService {
    request_tx: Sender<RecordingRequest>,
    #[allow(dead_code)]
    worker: JoinHandle<()>,
}

impl RecordingService {
    pub fn spawn() -> Self {
        let (request_tx, request_rx) = mpsc::channel();

        let worker = thread::spawn(move || {
            let mut controller = RecordingController::new();
            for request in request_rx {
                match request {
                    RecordingRequest::Start {
                        device_id,
                        recordings_dir,
                        reply,
                    } => {
                        let _ = reply.send(controller.start(&device_id, &recordings_dir));
                    }
                    RecordingRequest::Stop { reply } => {
                        let _ = reply.send(controller.stop());
                    }
                    RecordingRequest::Status { reply } => {
                        let _ = reply.send(controller.status());
                    }
                }
            }
        });

        Self { request_tx, worker }
    }

    fn send_start(
        &self,
        device_id: &str,
        recordings_dir: &Path,
    ) -> Result<RecordingStatus, AudioError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.request_tx
            .send(RecordingRequest::Start {
                device_id: device_id.to_string(),
                recordings_dir: recordings_dir.to_path_buf(),
                reply: reply_tx,
            })
            .map_err(|_| AudioError::Internal("service audio indisponible".into()))?;

        reply_rx
            .recv()
            .map_err(|_| AudioError::Internal("service audio indisponible".into()))?
    }

    fn send_stop(&self) -> Result<RecordingStatus, AudioError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.request_tx
            .send(RecordingRequest::Stop { reply: reply_tx })
            .map_err(|_| AudioError::Internal("service audio indisponible".into()))?;

        reply_rx
            .recv()
            .map_err(|_| AudioError::Internal("service audio indisponible".into()))?
    }

    fn send_status(&self) -> Result<RecordingStatus, AudioError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.request_tx
            .send(RecordingRequest::Status { reply: reply_tx })
            .map_err(|_| AudioError::Internal("service audio indisponible".into()))?;

        reply_rx
            .recv()
            .map_err(|_| AudioError::Internal("service audio indisponible".into()))
    }

    pub fn start(
        &self,
        device_id: &str,
        recordings_dir: &Path,
    ) -> Result<RecordingStatus, AudioError> {
        self.send_start(device_id, recordings_dir)
    }

    pub fn stop(&self) -> Result<RecordingStatus, AudioError> {
        self.send_stop()
    }

    pub fn status(&self) -> Result<RecordingStatus, AudioError> {
        self.send_status()
    }
}

fn map_config_error(message: String) -> AudioError {
    let lower = message.to_lowercase();
    if lower.contains("permission") || lower.contains("denied") {
        AudioError::PermissionDenied
    } else {
        AudioError::Internal(message)
    }
}

fn append_samples(buffer: &Arc<Mutex<Vec<f32>>>, data: &[f32]) {
    if let Ok(mut samples) = buffer.lock() {
        samples.extend_from_slice(data);
    }
}

fn build_input_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
    samples: Arc<Mutex<Vec<f32>>>,
    err_flag: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, AudioError> {
    let err_writer = Arc::clone(&err_flag);

    let stream = match sample_format {
        SampleFormat::F32 => {
            let writer = Arc::clone(&samples);
            device.build_input_stream(
                config,
                move |data: &[f32], _| append_samples(&writer, data),
                move |err| store_stream_error(&err_writer, err),
                None,
            )
        }
        SampleFormat::I16 => {
            let writer = Arc::clone(&samples);
            device.build_input_stream(
                config,
                move |data: &[i16], _| append_samples(&writer, &i16_to_f32(data)),
                move |err| store_stream_error(&err_writer, err),
                None,
            )
        }
        SampleFormat::U16 => {
            let writer = Arc::clone(&samples);
            device.build_input_stream(
                config,
                move |data: &[u16], _| append_samples(&writer, &u16_to_f32(data)),
                move |err| store_stream_error(&err_writer, err),
                None,
            )
        }
        other => {
            return Err(AudioError::Internal(format!(
                "format d'échantillon non supporté : {other:?}"
            )));
        }
    }
    .map_err(AudioError::from_cpal)?;

    Ok(stream)
}

fn store_stream_error(slot: &Arc<Mutex<Option<String>>>, err: cpal::StreamError) {
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(err.to_string());
    }
}

fn i16_to_f32(data: &[i16]) -> Vec<f32> {
    data.iter()
        .map(|sample| *sample as f32 / i16::MAX as f32)
        .collect()
}

fn u16_to_f32(data: &[u16]) -> Vec<f32> {
    data.iter()
        .map(|sample| (*sample as f32 / u16::MAX as f32) * 2.0 - 1.0)
        .collect()
}

fn write_wav(
    path: &Path,
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<(), AudioError> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: WavSampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec)?;
    for sample in samples {
        let scaled = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(scaled)?;
    }
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_status_from_service() {
        let service = RecordingService::spawn();
        let status = service.status().expect("status");
        assert_eq!(status.phase, RecordingPhase::Idle);
    }

    #[test]
    fn stop_without_start_returns_not_recording() {
        let service = RecordingService::spawn();
        assert!(matches!(service.stop(), Err(AudioError::NotRecording)));
    }
}
