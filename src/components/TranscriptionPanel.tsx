import { useCallback, useEffect, useState } from "react";
import { getAiSettings } from "../lib/ai/api";
import {
  getTranscriptionProgress,
  listenTranscriptionProgress,
  transcribeAudioFile,
  type Transcription,
  type TranscriptionProgress,
} from "../lib/transcription";

interface TranscriptionPanelProps {
  filePath: string | null;
  durationSecs?: number | null;
}

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

export function TranscriptionPanel({
  filePath,
  durationSecs,
}: TranscriptionPanelProps) {
  const [hasApiKey, setHasApiKey] = useState(false);
  const [progress, setProgress] = useState<TranscriptionProgress | null>(null);
  const [result, setResult] = useState<Transcription | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const refreshSettings = useCallback(async () => {
    try {
      const settings = await getAiSettings();
      setHasApiKey(settings.hasApiKey);
    } catch (err) {
      setError(formatError(err));
    }
  }, []);

  useEffect(() => {
    void refreshSettings();
    void getTranscriptionProgress().then(setProgress).catch(() => undefined);

    let unlisten: (() => void) | undefined;
    void listenTranscriptionProgress((nextProgress) => {
      setProgress(nextProgress);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [refreshSettings]);

  async function handleTranscribe() {
    if (!filePath) {
      return;
    }

    setError(null);
    setResult(null);
    setLoading(true);

    try {
      const transcription = await transcribeAudioFile({
        filePath,
        language: "fr",
        durationMs:
          durationSecs != null ? Math.round(durationSecs * 1000) : undefined,
      });
      setResult(transcription);
    } catch (err) {
      setError(formatError(err));
    } finally {
      setLoading(false);
      await refreshSettings();
    }
  }

  const isBusy =
    loading ||
    progress?.phase === "preparing" ||
    progress?.phase === "uploading" ||
    progress?.phase === "transcribing" ||
    progress?.phase === "saving";

  return (
    <section className="panel">
      <h3>Transcription Mistral</h3>

      {!hasApiKey && (
        <p className="warning">
          Configurez une clé API Mistral dans les réglages IA avant de transcrire.
        </p>
      )}

      <div className="row controls">
        <button
          type="button"
          onClick={() => void handleTranscribe()}
          disabled={!filePath || !hasApiKey || isBusy}
        >
          {isBusy ? "Transcription…" : "Transcrire"}
        </button>
      </div>

      {progress && progress.phase !== "idle" && (
        <p className="progress-message">{progress.message}</p>
      )}

      {result && (
        <div className="transcription-result">
          <h4>Résultat</h4>
          <p>{result.content}</p>
          {result.language && (
            <p className="meta">Langue détectée : {result.language}</p>
          )}
        </div>
      )}

      {error && <p className="error">{error}</p>}
    </section>
  );
}
