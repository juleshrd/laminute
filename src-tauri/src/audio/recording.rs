use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{self, sync_channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};
use uuid::Uuid;

use super::devices::resolve_input_device;
use super::error::AudioError;
use crate::ai::limits::MAX_AUDIO_BYTES;

const CHUNK_CHANNEL_CAPACITY: usize = 16;
const BUFFER_POOL_SIZE: usize = CHUNK_CHANNEL_CAPACITY + 1;
const CALLBACK_BUFFER_SAMPLES: usize = 65_536;
const WAV_HEADER_BYTES: u64 = 44;
const BYTES_PER_SAMPLE: u64 = std::mem::size_of::<i16>() as u64;
const FAILURE_NONE: u8 = 0;
const FAILURE_CHANNEL_SATURATED: u8 = 1;
const FAILURE_POOL_EXHAUSTED: u8 = 2;
const FAILURE_CALLBACK_TOO_LARGE: u8 = 3;
const FAILURE_SIZE_LIMIT: u8 = 4;
const FAILURE_STREAM_ERROR: u8 = 5;

type SampleBuffer = Vec<i16>;

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
    pub dropped_chunks: u64,
    pub dropped_samples: u64,
}

impl RecordingStatus {
    pub fn idle() -> Self {
        Self {
            phase: RecordingPhase::Idle,
            device_id: None,
            file_path: None,
            duration_secs: None,
            error: None,
            dropped_chunks: 0,
            dropped_samples: 0,
        }
    }
}

enum WriterMsg {
    Samples(SampleBuffer),
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordingDropMetrics {
    chunks: u64,
    samples: u64,
}

struct RecordingRuntimeState {
    failure_kind: AtomicU8,
    stop_requested: AtomicBool,
    dropped_chunks: AtomicU64,
    dropped_samples: AtomicU64,
    stream_error: Mutex<Option<String>>,
}

impl RecordingRuntimeState {
    fn new() -> Self {
        Self {
            failure_kind: AtomicU8::new(FAILURE_NONE),
            stop_requested: AtomicBool::new(false),
            dropped_chunks: AtomicU64::new(0),
            dropped_samples: AtomicU64::new(0),
            stream_error: Mutex::new(None),
        }
    }

    fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::Relaxed);
    }

    fn should_stop(&self) -> bool {
        self.stop_requested.load(Ordering::Relaxed)
    }

    fn mark_failure(&self, kind: u8, dropped_samples: u64) {
        if dropped_samples > 0 {
            self.dropped_chunks.fetch_add(1, Ordering::Relaxed);
            self.dropped_samples
                .fetch_add(dropped_samples, Ordering::Relaxed);
        }
        let _ = self.failure_kind.compare_exchange(
            FAILURE_NONE,
            kind,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
        self.request_stop();
    }

    fn store_stream_error(&self, message: String) {
        if let Ok(mut guard) = self.stream_error.lock() {
            if guard.is_none() {
                *guard = Some(message);
            }
        }
        self.mark_failure(FAILURE_STREAM_ERROR, 0);
    }

    fn metrics(&self) -> RecordingDropMetrics {
        RecordingDropMetrics {
            chunks: self.dropped_chunks.load(Ordering::Relaxed),
            samples: self.dropped_samples.load(Ordering::Relaxed),
        }
    }

    fn failure_error(&self) -> Option<AudioError> {
        match self.failure_kind.load(Ordering::Relaxed) {
            FAILURE_NONE => None,
            FAILURE_SIZE_LIMIT => Some(AudioError::RecordingTooLarge {
                max_mb: MAX_AUDIO_BYTES / 1024 / 1024,
            }),
            FAILURE_STREAM_ERROR => {
                let message = self
                    .stream_error
                    .lock()
                    .ok()
                    .and_then(|guard| guard.clone())
                    .unwrap_or_else(|| "erreur flux audio".into());
                Some(AudioError::Internal(message))
            }
            kind => Some(AudioError::Internal(self.failure_message(kind))),
        }
    }

    fn failure_message(&self, kind: u8) -> String {
        let metrics = self.metrics();
        let suffix = format!(
            " Le fichier partiel a été supprimé ({} bloc(s), {} échantillon(s) perdus).",
            metrics.chunks, metrics.samples
        );
        match kind {
            FAILURE_CHANNEL_SATURATED => {
                format!("tampon d'écriture saturé — enregistrement interrompu.{suffix}")
            }
            FAILURE_POOL_EXHAUSTED => {
                format!("tampons audio épuisés — enregistrement interrompu.{suffix}")
            }
            FAILURE_CALLBACK_TOO_LARGE => {
                format!("bloc audio trop volumineux — enregistrement interrompu.{suffix}")
            }
            _ => format!("enregistrement interrompu.{suffix}"),
        }
    }
}

struct ActiveRecording {
    #[allow(dead_code)]
    stream: cpal::Stream,
    chunk_tx: SyncSender<WriterMsg>,
    writer_handle: JoinHandle<Result<(), AudioError>>,
    runtime: Arc<RecordingRuntimeState>,
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
            let metrics = recording.runtime.metrics();
            return RecordingStatus {
                phase: RecordingPhase::Recording,
                device_id: Some(recording.device_id.clone()),
                file_path: Some(recording.output_path.to_string_lossy().into_owned()),
                duration_secs: Some(recording.started_at.elapsed().as_secs_f64()),
                error: recording.runtime.failure_error().map(|err| err.to_string()),
                dropped_chunks: metrics.chunks,
                dropped_samples: metrics.samples,
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

        let (output_path, output_file) = create_recording_file(recordings_dir)?;

        let spec = wav_spec(sample_rate, channels);
        let (chunk_tx, chunk_rx) = sync_channel(CHUNK_CHANNEL_CAPACITY);
        let (free_tx, free_rx) = sync_channel(BUFFER_POOL_SIZE);
        fill_buffer_pool(&free_tx)?;

        let runtime = Arc::new(RecordingRuntimeState::new());
        let writer_path = output_path.clone();
        let writer_runtime = Arc::clone(&runtime);
        let writer_free_tx = free_tx.clone();
        let writer_handle = thread::spawn(move || {
            run_disk_writer(
                output_file,
                writer_path,
                spec,
                chunk_rx,
                writer_free_tx,
                writer_runtime,
                MAX_AUDIO_BYTES,
            )
        });

        let stream = match build_input_stream(
            &device,
            &stream_config,
            sample_format,
            chunk_tx.clone(),
            free_tx,
            free_rx,
            Arc::clone(&runtime),
        ) {
            Ok(stream) => stream,
            Err(err) => {
                shutdown_failed_start(chunk_tx, writer_handle, &output_path);
                return Err(err);
            }
        };

        if let Err(err) = stream.play() {
            drop(stream);
            shutdown_failed_start(chunk_tx, writer_handle, &output_path);
            return Err(AudioError::Internal(err.to_string()));
        }

        self.active = Some(ActiveRecording {
            stream,
            chunk_tx,
            writer_handle,
            runtime,
            started_at: Instant::now(),
            output_path,
            device_id: device_id.to_string(),
        });

        Ok(self.status())
    }

    fn stop(&mut self) -> Result<RecordingStatus, AudioError> {
        let recording = self.active.take().ok_or(AudioError::NotRecording)?;
        self.finish_recording(recording)
    }

    fn stop_if_failed(&mut self) {
        let should_stop = self
            .active
            .as_ref()
            .is_some_and(|recording| recording.runtime.should_stop());

        if should_stop {
            if let Some(recording) = self.active.take() {
                let _ = self.finish_recording(recording);
            }
        }
    }

    fn finish_recording(
        &mut self,
        recording: ActiveRecording,
    ) -> Result<RecordingStatus, AudioError> {
        let duration_secs = recording.started_at.elapsed().as_secs_f64();
        let output_path = recording.output_path.clone();
        let device_id = recording.device_id.clone();
        let runtime = Arc::clone(&recording.runtime);

        let result = finalize_recording(
            recording.stream,
            recording.chunk_tx,
            recording.writer_handle,
            runtime,
            &output_path,
        );

        let metrics = recording.runtime.metrics();

        if let Err(err) = result {
            self.last_status = RecordingStatus {
                phase: RecordingPhase::Stopped,
                device_id: Some(device_id),
                file_path: None,
                duration_secs: Some(duration_secs),
                error: Some(err.to_string()),
                dropped_chunks: metrics.chunks,
                dropped_samples: metrics.samples,
            };
            return Err(err);
        }

        let file_path = output_path.to_string_lossy().into_owned();
        self.last_status = RecordingStatus {
            phase: RecordingPhase::Stopped,
            device_id: Some(device_id),
            file_path: Some(file_path),
            duration_secs: Some(duration_secs),
            error: None,
            dropped_chunks: metrics.chunks,
            dropped_samples: metrics.samples,
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
            loop {
                controller.stop_if_failed();

                let request = match request_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(request) => request,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };

                controller.stop_if_failed();

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
    runtime: &RecordingRuntimeState,
    output_path: &Path,
) -> Result<(), AudioError> {
    let writer_result =
        writer_join.map_err(|_| AudioError::Internal("thread d'écriture interrompu".into()))?;

    if let Err(err) = writer_result {
        let _ = fs::remove_file(output_path);
        return Err(err);
    }

    if let Some(err) = runtime.failure_error() {
        let _ = fs::remove_file(output_path);
        return Err(err);
    }

    Ok(())
}

fn finalize_recording(
    stream: cpal::Stream,
    chunk_tx: SyncSender<WriterMsg>,
    writer_handle: JoinHandle<Result<(), AudioError>>,
    runtime: Arc<RecordingRuntimeState>,
    output_path: &Path,
) -> Result<(), AudioError> {
    runtime.request_stop();
    drop(stream);

    let _ = chunk_tx.send(WriterMsg::Shutdown);

    let writer_join = writer_handle.join();

    complete_after_writer_join(writer_join, &runtime, output_path)
}

fn run_disk_writer(
    file: File,
    path: PathBuf,
    spec: WavSpec,
    rx: Receiver<WriterMsg>,
    free_tx: SyncSender<SampleBuffer>,
    runtime: Arc<RecordingRuntimeState>,
    max_audio_bytes: u64,
) -> Result<(), AudioError> {
    let mut writer = WavWriter::new(file, spec)?;
    let mut data_bytes = 0_u64;

    while let Ok(msg) = rx.recv() {
        match msg {
            WriterMsg::Samples(chunk) => {
                let chunk_bytes = (chunk.len() as u64).saturating_mul(BYTES_PER_SAMPLE);
                let next_size = WAV_HEADER_BYTES
                    .saturating_add(data_bytes)
                    .saturating_add(chunk_bytes);

                if next_size > max_audio_bytes {
                    runtime.mark_failure(FAILURE_SIZE_LIMIT, 0);
                    return_buffer(&free_tx, chunk);
                    return Err(AudioError::RecordingTooLarge {
                        max_mb: MAX_AUDIO_BYTES / 1024 / 1024,
                    });
                }

                for &sample in &chunk {
                    writer.write_sample(sample)?;
                }
                data_bytes = data_bytes.saturating_add(chunk_bytes);
                return_buffer(&free_tx, chunk);
            }
            WriterMsg::Shutdown => break,
        }
    }

    if let Err(err) = writer.finalize() {
        let _ = fs::remove_file(&path);
        return Err(err.into());
    }

    Ok(())
}

fn create_recording_file(recordings_dir: &Path) -> Result<(PathBuf, File), AudioError> {
    for _ in 0..16 {
        let output_path = recordings_dir.join(format!("{}.wav", Uuid::new_v4()));
        match create_new_file(&output_path) {
            Ok(file) => return Ok((output_path, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(AudioError::Io(err.to_string())),
        }
    }

    Err(AudioError::Internal(
        "impossible de créer un nom d'enregistrement unique".into(),
    ))
}

fn create_new_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn fill_buffer_pool(free_tx: &SyncSender<SampleBuffer>) -> Result<(), AudioError> {
    for _ in 0..BUFFER_POOL_SIZE {
        free_tx
            .send(Vec::with_capacity(CALLBACK_BUFFER_SAMPLES))
            .map_err(|_| AudioError::Internal("tampons audio indisponibles".into()))?;
    }
    Ok(())
}

fn shutdown_failed_start(
    chunk_tx: SyncSender<WriterMsg>,
    writer_handle: JoinHandle<Result<(), AudioError>>,
    output_path: &Path,
) {
    let _ = chunk_tx.send(WriterMsg::Shutdown);
    let _ = writer_handle.join();
    let _ = fs::remove_file(output_path);
}

fn return_buffer(free_tx: &SyncSender<SampleBuffer>, mut buffer: SampleBuffer) {
    buffer.clear();
    let _ = free_tx.try_send(buffer);
}

fn with_callback_buffer<F>(
    sample_count: usize,
    tx: &SyncSender<WriterMsg>,
    free_tx: &SyncSender<SampleBuffer>,
    free_rx: &Receiver<SampleBuffer>,
    runtime: &RecordingRuntimeState,
    fill: F,
) where
    F: FnOnce(&mut SampleBuffer),
{
    if runtime.should_stop() {
        return;
    }

    if sample_count > CALLBACK_BUFFER_SAMPLES {
        runtime.mark_failure(FAILURE_CALLBACK_TOO_LARGE, sample_count as u64);
        return;
    }

    let mut buffer = match free_rx.try_recv() {
        Ok(buffer) => buffer,
        Err(mpsc::TryRecvError::Empty) => {
            runtime.mark_failure(FAILURE_POOL_EXHAUSTED, sample_count as u64);
            return;
        }
        Err(mpsc::TryRecvError::Disconnected) => return,
    };

    if buffer.capacity() < sample_count {
        runtime.mark_failure(FAILURE_CALLBACK_TOO_LARGE, sample_count as u64);
        return_buffer(free_tx, buffer);
        return;
    }

    buffer.clear();
    fill(&mut buffer);

    match tx.try_send(WriterMsg::Samples(buffer)) {
        Ok(()) => {}
        Err(mpsc::TrySendError::Full(WriterMsg::Samples(buffer))) => {
            let dropped_samples = buffer.len() as u64;
            runtime.mark_failure(FAILURE_CHANNEL_SATURATED, dropped_samples);
            return_buffer(free_tx, buffer);
        }
        Err(mpsc::TrySendError::Full(WriterMsg::Shutdown)) => {}
        Err(mpsc::TrySendError::Disconnected(WriterMsg::Samples(buffer))) => {
            return_buffer(free_tx, buffer);
        }
        Err(mpsc::TrySendError::Disconnected(WriterMsg::Shutdown)) => {}
    }
}

fn send_f32_samples(
    data: &[f32],
    tx: &SyncSender<WriterMsg>,
    free_tx: &SyncSender<SampleBuffer>,
    free_rx: &Receiver<SampleBuffer>,
    runtime: &RecordingRuntimeState,
) {
    with_callback_buffer(data.len(), tx, free_tx, free_rx, runtime, |buffer| {
        for &sample in data {
            buffer.push((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        }
    });
}

fn send_i16_samples(
    data: &[i16],
    tx: &SyncSender<WriterMsg>,
    free_tx: &SyncSender<SampleBuffer>,
    free_rx: &Receiver<SampleBuffer>,
    runtime: &RecordingRuntimeState,
) {
    with_callback_buffer(data.len(), tx, free_tx, free_rx, runtime, |buffer| {
        buffer.extend_from_slice(data);
    });
}

fn send_u16_samples(
    data: &[u16],
    tx: &SyncSender<WriterMsg>,
    free_tx: &SyncSender<SampleBuffer>,
    free_rx: &Receiver<SampleBuffer>,
    runtime: &RecordingRuntimeState,
) {
    with_callback_buffer(data.len(), tx, free_tx, free_rx, runtime, |buffer| {
        for &sample in data {
            let normalized = (sample as f32 / u16::MAX as f32) * 2.0 - 1.0;
            buffer.push((normalized.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        }
    });
}

fn store_stream_error(state: &Arc<RecordingRuntimeState>, err: cpal::StreamError) {
    state.store_stream_error(err.to_string());
}

fn build_input_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
    chunk_tx: SyncSender<WriterMsg>,
    free_tx: SyncSender<SampleBuffer>,
    free_rx: Receiver<SampleBuffer>,
    runtime: Arc<RecordingRuntimeState>,
) -> Result<cpal::Stream, AudioError> {
    let err_writer = Arc::clone(&runtime);

    let stream = match sample_format {
        SampleFormat::F32 => {
            let tx = chunk_tx;
            let free_tx = free_tx;
            let state = Arc::clone(&runtime);
            device.build_input_stream(
                config,
                move |data: &[f32], _| send_f32_samples(data, &tx, &free_tx, &free_rx, &state),
                move |err| store_stream_error(&err_writer, err),
                None,
            )
        }
        SampleFormat::I16 => {
            let tx = chunk_tx;
            let free_tx = free_tx;
            let state = Arc::clone(&runtime);
            device.build_input_stream(
                config,
                move |data: &[i16], _| send_i16_samples(data, &tx, &free_tx, &free_rx, &state),
                move |err| store_stream_error(&err_writer, err),
                None,
            )
        }
        SampleFormat::U16 => {
            let tx = chunk_tx;
            let free_tx = free_tx;
            let state = Arc::clone(&runtime);
            device.build_input_stream(
                config,
                move |data: &[u16], _| send_u16_samples(data, &tx, &free_tx, &free_rx, &state),
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

    struct WriterHarness {
        tx: SyncSender<WriterMsg>,
        handle: JoinHandle<Result<(), AudioError>>,
        runtime: Arc<RecordingRuntimeState>,
    }

    fn spawn_disk_writer(path: &Path, spec: WavSpec) -> WriterHarness {
        spawn_disk_writer_with_limit(path, spec, MAX_AUDIO_BYTES)
    }

    fn spawn_disk_writer_with_limit(
        path: &Path,
        spec: WavSpec,
        max_audio_bytes: u64,
    ) -> WriterHarness {
        let (tx, rx) = sync_channel(CHUNK_CHANNEL_CAPACITY);
        let (free_tx, _free_rx) = sync_channel(BUFFER_POOL_SIZE);
        let runtime = Arc::new(RecordingRuntimeState::new());
        let writer_runtime = Arc::clone(&runtime);
        let path = path.to_path_buf();
        let file = create_new_file(&path).expect("create wav");
        let handle = thread::spawn(move || {
            run_disk_writer(
                file,
                path,
                spec,
                rx,
                free_tx,
                writer_runtime,
                max_audio_bytes,
            )
        });
        WriterHarness {
            tx,
            handle,
            runtime,
        }
    }

    fn finish_disk_writer(harness: WriterHarness, path: &Path) -> Result<(), AudioError> {
        let _ = harness.tx.send(WriterMsg::Shutdown);
        let writer_join = harness.handle.join();
        complete_after_writer_join(writer_join, &harness.runtime, path)
    }

    fn finish_failed_writer(harness: WriterHarness, path: &Path) -> Result<(), AudioError> {
        drop(harness.tx);
        let writer_join = harness.handle.join();
        complete_after_writer_join(writer_join, &harness.runtime, path)
    }

    type CallbackChannels = (
        SyncSender<WriterMsg>,
        Receiver<WriterMsg>,
        SyncSender<SampleBuffer>,
        Receiver<SampleBuffer>,
        Arc<RecordingRuntimeState>,
    );

    fn callback_channels(capacity: usize) -> CallbackChannels {
        let (tx, rx) = sync_channel(capacity);
        let (free_tx, free_rx) = sync_channel(BUFFER_POOL_SIZE);
        free_tx
            .send(Vec::with_capacity(CALLBACK_BUFFER_SAMPLES))
            .expect("seed buffer");
        (
            tx,
            rx,
            free_tx,
            free_rx,
            Arc::new(RecordingRuntimeState::new()),
        )
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
        let harness = spawn_disk_writer(&path, spec);

        let chunk: Vec<i16> = (0..512).map(|i| (i % i16::MAX as usize) as i16).collect();
        harness
            .tx
            .send(WriterMsg::Samples(chunk.clone()))
            .expect("chunk 1");
        harness
            .tx
            .send(WriterMsg::Samples(chunk.clone()))
            .expect("chunk 2");
        harness.tx.send(WriterMsg::Samples(chunk)).expect("chunk 3");

        finish_disk_writer(harness, &path).expect("finalize");

        let reader = WavReader::open(&path).expect("open wav");
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.spec().sample_rate, 48_000);
        assert_eq!(reader.len(), 512 * 3);
    }

    #[test]
    fn recording_file_uses_uuid_and_exclusive_create() {
        let dir = tempdir().expect("tempdir");
        let (first_path, _first_file) = create_recording_file(dir.path()).expect("first file");
        let (second_path, _second_file) = create_recording_file(dir.path()).expect("second file");

        assert_ne!(first_path, second_path);
        assert_eq!(
            first_path.extension().and_then(|ext| ext.to_str()),
            Some("wav")
        );
        assert!(first_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| Uuid::parse_str(stem).ok())
            .is_some());

        fs::write(&first_path, b"keep me").expect("seed existing file");
        let err = create_new_file(&first_path).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&first_path).expect("read existing"), b"keep me");
    }

    #[test]
    fn callback_reuses_preallocated_buffer_capacity() {
        let (tx, rx, free_tx, free_rx, runtime) = callback_channels(1);
        let input = [1_i16, -2, 3, -4];

        for _ in 0..3 {
            send_i16_samples(&input, &tx, &free_tx, &free_rx, &runtime);
            let WriterMsg::Samples(mut buffer) = rx.recv().expect("samples") else {
                panic!("unexpected shutdown");
            };
            assert_eq!(buffer, input);
            assert_eq!(buffer.capacity(), CALLBACK_BUFFER_SAMPLES);
            buffer.clear();
            free_tx.send(buffer).expect("return buffer");
        }

        assert!(runtime.failure_error().is_none());
    }

    #[test]
    fn full_chunk_channel_sets_stop_and_drop_metrics() {
        let (tx, _rx, free_tx, free_rx, runtime) = callback_channels(1);
        tx.send(WriterMsg::Samples(vec![0; 4]))
            .expect("fill channel");

        send_i16_samples(&[1, 2, 3, 4], &tx, &free_tx, &free_rx, &runtime);

        let err = runtime.failure_error().expect("failure");
        let metrics = runtime.metrics();
        assert!(runtime.should_stop());
        assert!(err.to_string().contains("saturé"));
        assert_eq!(metrics.chunks, 1);
        assert_eq!(metrics.samples, 4);
    }

    #[test]
    fn runtime_error_after_finalize_removes_wav() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.wav");
        let spec = wav_spec(44_100, 1);
        let harness = spawn_disk_writer(&path, spec);
        harness
            .tx
            .send(WriterMsg::Samples(vec![100, -100, 0]))
            .expect("chunk");
        let runtime = Arc::clone(&harness.runtime);
        finish_disk_writer(harness, &path).expect("finalize");
        runtime.mark_failure(FAILURE_CHANNEL_SATURATED, 3);

        let result = complete_after_writer_join(Ok(Ok(())), &runtime, &path);
        assert!(matches!(result, Err(AudioError::Internal(_))));
        assert!(!path.exists());
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
            &RecordingRuntimeState::new(),
            &path,
        );
        assert!(matches!(result, Err(AudioError::Internal(_))));
        assert!(!path.exists());
    }

    #[test]
    fn size_limit_stops_and_removes_partial_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("limited.wav");
        let spec = wav_spec(48_000, 1);
        let harness = spawn_disk_writer_with_limit(&path, spec, WAV_HEADER_BYTES + 4);

        harness
            .tx
            .send(WriterMsg::Samples(vec![1, 2, 3]))
            .expect("oversized chunk");

        let result = finish_failed_writer(harness, &path);
        assert!(matches!(result, Err(AudioError::RecordingTooLarge { .. })));
        assert!(!path.exists());
    }

    #[test]
    fn synthetic_writer_soak_keeps_disk_usage_bounded() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("soak.wav");
        let sample_rate = 8_000;
        let spec = wav_spec(sample_rate, 1);
        let harness = spawn_disk_writer(&path, spec);
        let chunk = vec![0_i16; 800];
        let chunks = 1_200_u64;

        for _ in 0..chunks {
            harness
                .tx
                .send(WriterMsg::Samples(chunk.clone()))
                .expect("soak chunk");
        }

        finish_disk_writer(harness, &path).expect("finalize soak");

        let metadata = fs::metadata(&path).expect("metadata");
        let expected_max = WAV_HEADER_BYTES + chunks * chunk.len() as u64 * BYTES_PER_SAMPLE;
        assert!(metadata.len() <= expected_max);

        let reader = WavReader::open(&path).expect("open soak wav");
        assert_eq!(reader.spec().sample_rate, sample_rate);
        assert_eq!(reader.len(), chunk.len() as u32 * chunks as u32);
    }
}
