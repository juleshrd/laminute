import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  type AudioInputDevice,
  type RecordingStatus,
  formatAudioError,
  formatDuration,
} from "../lib/audio";
import { TranscriptionPanel } from "./TranscriptionPanel";

function formatError(error: unknown): string {
  return formatAudioError(error);
}

export function MicrophoneRecorder() {
  const [devices, setDevices] = useState<AudioInputDevice[]>([]);
  const [selectedDeviceId, setSelectedDeviceId] = useState("");
  const [status, setStatus] = useState<RecordingStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refreshDevices = useCallback(async () => {
    setError(null);
    try {
      const listed = await invoke<AudioInputDevice[]>("list_audio_input_devices");
      setDevices(listed);

      const selected = await invoke<AudioInputDevice | null>("get_selected_audio_input_device");
      if (selected) {
        setSelectedDeviceId(selected.id);
      } else if (listed.length > 0) {
        const fallback = listed.find((device) => device.isDefault) ?? listed[0];
        setSelectedDeviceId(fallback.id);
      }
    } catch (err) {
      setError(formatError(err));
    }
  }, []);

  const refreshStatus = useCallback(async () => {
    try {
      const nextStatus = await invoke<RecordingStatus>("get_recording_status");
      setStatus(nextStatus);
    } catch (err) {
      setError(formatError(err));
    }
  }, []);

  useEffect(() => {
    void (async () => {
      setLoading(true);
      await refreshDevices();
      await refreshStatus();
      setLoading(false);
    })();
  }, [refreshDevices, refreshStatus]);

  useEffect(() => {
    if (status?.phase !== "recording") {
      return;
    }

    const timer = window.setInterval(() => {
      void refreshStatus();
    }, 500);

    return () => window.clearInterval(timer);
  }, [refreshStatus, status?.phase]);

  async function handleSelectDevice() {
    if (!selectedDeviceId) {
      return;
    }

    setError(null);
    try {
      const selected = await invoke<AudioInputDevice>("set_selected_audio_input_device", {
        deviceId: selectedDeviceId,
      });
      setSelectedDeviceId(selected.id);
    } catch (err) {
      setError(formatError(err));
    }
  }

  async function handleStartRecording() {
    setError(null);
    try {
      const nextStatus = await invoke<RecordingStatus>("start_microphone_recording");
      setStatus(nextStatus);
    } catch (err) {
      setError(formatError(err));
      await refreshStatus();
    }
  }

  async function handleStopRecording() {
    setError(null);
    try {
      const nextStatus = await invoke<RecordingStatus>("stop_microphone_recording");
      setStatus(nextStatus);
    } catch (err) {
      setError(formatError(err));
      await refreshStatus();
    }
  }

  const isRecording = status?.phase === "recording";

  return (
    <div className="audio-test">
      <h2>Microphone</h2>

      {loading ? (
        <p>Chargement des périphériques…</p>
      ) : (
        <section className="panel">
          <h3>Périphérique d&apos;entrée</h3>
          {devices.length === 0 ? (
            <p className="warning">Aucun périphérique d&apos;entrée détecté.</p>
          ) : (
            <div className="row controls">
              <select
                value={selectedDeviceId}
                onChange={(event) => setSelectedDeviceId(event.currentTarget.value)}
                disabled={isRecording}
              >
                {devices.map((device) => (
                  <option key={device.id} value={device.id}>
                    {device.name}
                    {device.isDefault ? " (défaut)" : ""}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={() => void handleSelectDevice()}
                disabled={isRecording}
              >
                Mémoriser
              </button>
              <button type="button" onClick={() => void refreshDevices()} disabled={isRecording}>
                Actualiser
              </button>
            </div>
          )}
        </section>
      )}

      <section className="panel">
        <h3>Enregistrement</h3>
        <div className="row controls">
          <button
            type="button"
            onClick={() => void handleStartRecording()}
            disabled={isRecording || !selectedDeviceId}
          >
            Démarrer
          </button>
          <button type="button" onClick={() => void handleStopRecording()} disabled={!isRecording}>
            Arrêter
          </button>
        </div>

        {status && (
          <dl className="status-grid">
            <div>
              <dt>Statut</dt>
              <dd>{status.phase}</dd>
            </div>
            <div>
              <dt>Durée</dt>
              <dd>{formatDuration(status.durationSecs)}</dd>
            </div>
            <div>
              <dt>Fichier</dt>
              <dd className="mono">{status.filePath ?? "—"}</dd>
            </div>
          </dl>
        )}
      </section>

      <TranscriptionPanel filePath={status?.filePath ?? null} durationSecs={status?.durationSecs} />

      {error && <p className="error">{error}</p>}
    </div>
  );
}
