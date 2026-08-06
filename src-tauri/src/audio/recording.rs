use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, sync_channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};

use super::devices::resolve_input_device;
use super::error::AudioError;

const CHUNK_CHANNEL_CAPACITY: usize = 16;

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

enum WriterMsg {
    Samples(Vec<i16>),
    Shutdown,
}

struct ActiveRecording {
    #[allow(dead_code)]
    stream: cpal::Stream,
    chunk_tx: SyncSender<WriterMsg>,
    writer_handle: JoinHandle<Result<(), AudioError>>,
    err_flag: Arc<Mutex<Option<String>>>,
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

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let output_path = recordings_dir.join(format!("recording-{timestamp}.wav"));

        let spec = wav_spec(sample_rate, channels);
        let (chunk_tx, chunk_rx) = sync_channel(CHUNK_CHANNEL_CAPACITY);
        let writer_path = output_path.clone();
        let writer_handle = thread::spawn(move || run_disk_writer(writer_path, spec, chunk_rx));

        let err_flag = Arc::new(Mutex::new(None::<String>));

        let stream = build_input_stream(
            &device,
            &stream_config,
            sample_format,
            chunk_tx.clone(),
            Arc::clone(&err_flag),
        )?;

        stream
            .play()
            .map_err(|err| AudioError::Internal(err.to_string()))?;

        self.active = Some(ActiveRecording {
            stream,
            chunk_tx,
            writer_handle,
            err_flag,
            started_at: Instant::now(),
            output_path,
            device_id: device_id.to_string(),
        });

        Ok(self.status())
    }

    fn stop(&mut self) -> Result<RecordingStatus, AudioError> {
        let recording = self.active.take().ok_or(AudioError::NotRecording)?;

        let duration_secs = recording.started_at.elapsed().as_secs_f64();
        let output_path = recording.output_path.clone();
        let device_id = recording.device_id;

        finalize_recording(
            recording.stream,
            recording.chunk_tx,
            recording.writer_handle,
            recording.err_flag,
            &output_path,
        )?;

        let file_path = output_path.to_string_lossy().into_owned();
        self.last_status = RecordingStatus {
            phase: RecordingPhase::Stopped,
            device_id: Some(device_id),
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

fn wav_spec(sample_rate: u32, channels: u16) -> WavSpec {
    WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: WavSampleFormat::Int,
    }
}

fn complete_after_writer_join(
    writer_join: thread::Result<Result<(), AudioError>>,
    stream_error: Option<String>,
    output_path: &Path,
) -> Result<(), AudioError> {
    let writer_result =
        writer_join.map_err(|_| AudioError::Internal("thread d'écriture interrompu".into()))?;

    if let Err(err) = writer_result {
        let _ = fs::remove_file(output_path);
        return Err(err);
    }

    if let Some(message) = stream_error {
        return Err(AudioError::Internal(message));
    }

    Ok(())
}

fn finalize_recording(
    stream: cpal::Stream,
    chunk_tx: SyncSender<WriterMsg>,
    writer_handle: JoinHandle<Result<(), AudioError>>,
    err_flag: Arc<Mutex<Option<String>>>,
    output_path: &Path,
) -> Result<(), AudioError> {
    drop(stream);

    let _ = chunk_tx.send(WriterMsg::Shutdown);

    let writer_join = writer_handle.join();
    let stream_error = err_flag
        .lock()
        .map_err(|_| AudioError::Internal("verrou audio indisponible".into()))?
        .take();

    complete_after_writer_join(writer_join, stream_error, output_path)
}

fn run_disk_writer(
    path: PathBuf,
    spec: WavSpec,
    rx: Receiver<WriterMsg>,
) -> Result<(), AudioError> {
    let mut writer = WavWriter::create(&path, spec)?;
    while let Ok(msg) = rx.recv() {
        match msg {
            WriterMsg::Samples(chunk) => {
                for sample in chunk {
                    writer.write_sample(sample)?;
                }
            }
            WriterMsg::Shutdown => break,
        }
    }
    writer.finalize()?;
    Ok(())
}

fn try_send_chunk(
    tx: &SyncSender<WriterMsg>,
    err_flag: &Arc<Mutex<Option<String>>>,
    chunk: Vec<i16>,
) {
    match tx.try_send(WriterMsg::Samples(chunk)) {
        Ok(()) => {}
        // Never block the realtime callback; record an error instead.
        Err(mpsc::TrySendError::Full(_)) => store_recording_error(
            err_flag,
            "tampon d'écriture saturé — enregistrement interrompu",
        ),
        Err(mpsc::TrySendError::Disconnected(_)) => {}
    }
}

fn store_recording_error(err_flag: &Arc<Mutex<Option<String>>>, message: &str) {
    if let Ok(mut guard) = err_flag.lock() {
        if guard.is_none() {
            *guard = Some(message.to_string());
        }
    }
}

fn store_stream_error(slot: &Arc<Mutex<Option<String>>>, err: cpal::StreamError) {
    if let Ok(mut guard) = slot.lock() {
        if guard.is_none() {
            *guard = Some(err.to_string());
        }
    }
}

fn f32_to_i16_chunk(data: &[f32]) -> Vec<i16> {
    let mut out = Vec::with_capacity(data.len());
    for &sample in data {
        out.push((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
    }
    out
}

fn u16_to_i16_chunk(data: &[u16]) -> Vec<i16> {
    let mut out = Vec::with_capacity(data.len());
    for &sample in data {
        let normalized = (sample as f32 / u16::MAX as f32) * 2.0 - 1.0;
        out.push((normalized.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
    }
    out
}

fn build_input_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
    chunk_tx: SyncSender<WriterMsg>,
    err_flag: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, AudioError> {
    let err_writer = Arc::clone(&err_flag);

    let stream = match sample_format {
        SampleFormat::F32 => {
            let tx = chunk_tx;
            let flag = Arc::clone(&err_flag);
            device.build_input_stream(
                config,
                move |data: &[f32], _| try_send_chunk(&tx, &flag, f32_to_i16_chunk(data)),
                move |err| store_stream_error(&err_writer, err),
                None,
            )
        }
        SampleFormat::I16 => {
            let tx = chunk_tx;
            let flag = Arc::clone(&err_flag);
            device.build_input_stream(
                config,
                move |data: &[i16], _| try_send_chunk(&tx, &flag, data.to_vec()),
                move |err| store_stream_error(&err_writer, err),
                None,
            )
        }
        SampleFormat::U16 => {
            let tx = chunk_tx;
            let flag = Arc::clone(&err_flag);
            device.build_input_stream(
                config,
                move |data: &[u16], _| try_send_chunk(&tx, &flag, u16_to_i16_chunk(data)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use hound::WavReader;
    use tempfile::tempdir;

    fn spawn_disk_writer(
        path: &Path,
        spec: WavSpec,
    ) -> (SyncSender<WriterMsg>, JoinHandle<Result<(), AudioError>>) {
        let (tx, rx) = sync_channel(CHUNK_CHANNEL_CAPACITY);
        let path = path.to_path_buf();
        let handle = thread::spawn(move || run_disk_writer(path, spec, rx));
        (tx, handle)
    }

    fn finish_disk_writer(
        tx: SyncSender<WriterMsg>,
        handle: JoinHandle<Result<(), AudioError>>,
    ) -> Result<(), AudioError> {
        let _ = tx.send(WriterMsg::Shutdown);
        handle
            .join()
            .map_err(|_| AudioError::Internal("thread d'écriture interrompu".into()))?
    }

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

    #[test]
    fn disk_writer_finalizes_valid_wav() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.wav");
        let spec = wav_spec(48_000, 2);
        let (tx, handle) = spawn_disk_writer(&path, spec);

        let chunk: Vec<i16> = (0..512).map(|i| (i % i16::MAX as usize) as i16).collect();
        tx.send(WriterMsg::Samples(chunk.clone())).expect("chunk 1");
        tx.send(WriterMsg::Samples(chunk.clone())).expect("chunk 2");
        tx.send(WriterMsg::Samples(chunk)).expect("chunk 3");

        finish_disk_writer(tx, handle).expect("finalize");

        let reader = WavReader::open(&path).expect("open wav");
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.spec().sample_rate, 48_000);
        assert_eq!(reader.len(), 512 * 3);
    }

    #[test]
    fn full_chunk_channel_sets_err_flag() {
        let err_flag = Arc::new(Mutex::new(None));
        let (tx, _rx) = sync_channel(1);
        tx.send(WriterMsg::Samples(vec![0; 4]))
            .expect("fill channel");

        try_send_chunk(&tx, &err_flag, vec![1; 4]);

        let message = err_flag.lock().expect("lock").clone();
        assert!(message.is_some());
        assert!(message.unwrap().contains("saturé"));
    }

    #[test]
    fn stream_error_after_finalize_returns_error_but_keeps_wav() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.wav");
        let spec = wav_spec(44_100, 1);
        let (tx, handle) = spawn_disk_writer(&path, spec);
        tx.send(WriterMsg::Samples(vec![100, -100, 0]))
            .expect("chunk");
        finish_disk_writer(tx, handle).expect("finalize");

        let result =
            complete_after_writer_join(Ok(Ok(())), Some("erreur flux audio simulée".into()), &path);
        assert!(matches!(result, Err(AudioError::Internal(_))));
        assert!(path.exists());

        let reader = WavReader::open(&path).expect("open wav");
        assert_eq!(reader.len(), 3);
    }

    #[test]
    fn writer_failure_removes_partial_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("partial.wav");
        fs::write(&path, b"RIFF").expect("seed partial");

        let result = complete_after_writer_join(
            Ok(Err(AudioError::Internal(
                "échec de finalisation simulé".into(),
            ))),
            None,
            &path,
        );
        assert!(matches!(result, Err(AudioError::Internal(_))));
        assert!(!path.exists());
    }
}
