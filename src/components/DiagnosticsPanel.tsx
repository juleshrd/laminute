import { useCallback, useEffect, useState } from "react";

import {
  getDiagnosticsSnapshot,
  previewSupportBundle,
  saveSupportBundle,
  type DiagnosticsSnapshot,
  type SupportBundlePreview,
} from "../lib/diagnostics";
import { formatBytes } from "../lib/privacy";

export function DiagnosticsPanel() {
  const [snapshot, setSnapshot] = useState<DiagnosticsSnapshot | null>(null);
  const [preview, setPreview] = useState<SupportBundlePreview | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const next = await getDiagnosticsSnapshot();
      setSnapshot(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Chargement du diagnostic impossible.");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handlePreview() {
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const next = await previewSupportBundle();
      setPreview(next);
      setStatus("Aperçu du bundle prêt — vérifiez le contenu avant enregistrement.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Aperçu impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function handleSave() {
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      if (!preview) {
        const next = await previewSupportBundle();
        setPreview(next);
      }
      const saved = await saveSupportBundle();
      setStatus(
        saved
          ? "Bundle de support enregistré."
          : "Enregistrement annulé — aucun fichier écrit.",
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "Enregistrement impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function handleCopyReport() {
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const next = preview ?? (await previewSupportBundle());
      setPreview(next);
      await navigator.clipboard.writeText(next.githubReport);
      setStatus("Rapport court copié dans le presse-papiers.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Copie impossible.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="diagnostics-panel" aria-labelledby="diagnostics-panel-title">
      <header className="diagnostics-panel__header">
        <h2 id="diagnostics-panel-title">Diagnostic et support</h2>
        <p>
          État local de l&apos;application. Les journaux et le bundle volontaire n&apos;incluent ni
          clé API, ni transcription, ni audio.
        </p>
      </header>

      {loading && <p className="diagnostics-panel__loading">Chargement…</p>}

      {snapshot && (
        <dl className="status-grid diagnostics-panel__grid">
          <div>
            <dt>Version</dt>
            <dd>{snapshot.appVersion}</dd>
          </div>
          <div>
            <dt>Système</dt>
            <dd>
              {snapshot.os} / {snapshot.arch}
            </dd>
          </div>
          <div>
            <dt>Schéma DB</dt>
            <dd>{snapshot.dbSchemaVersion ?? "—"}</dd>
          </div>
          <div>
            <dt>Fournisseur</dt>
            <dd>{snapshot.providerId ?? "aucun"}</dd>
          </div>
          <div>
            <dt>Modèle transcription</dt>
            <dd>{snapshot.transcriptionModel ?? "—"}</dd>
          </div>
          <div>
            <dt>Modèle compte-rendu</dt>
            <dd>{snapshot.summaryModel ?? "—"}</dd>
          </div>
          <div>
            <dt>Trousseau</dt>
            <dd>{snapshot.keyringStatus}</dd>
          </div>
          <div>
            <dt>Microphone</dt>
            <dd>{snapshot.microphoneStatus}</dd>
          </div>
          <div>
            <dt>Mises à jour</dt>
            <dd>{snapshot.updaterStatus}</dd>
          </div>
          <div>
            <dt>Données</dt>
            <dd className="mono">{snapshot.appDataDir}</dd>
          </div>
          <div>
            <dt>Journaux</dt>
            <dd className="mono">{snapshot.logsDir}</dd>
          </div>
          <div>
            <dt>Base</dt>
            <dd className="mono">{snapshot.dbPath}</dd>
          </div>
        </dl>
      )}

      {snapshot && snapshot.recentErrors.length > 0 ? (
        <div className="diagnostics-panel__errors">
          <h3>Derniers codes d&apos;erreur</h3>
          <ul>
            {snapshot.recentErrors
              .slice()
              .reverse()
              .slice(0, 8)
              .map((event) => (
                <li key={`${event.timestamp}-${event.code}-${event.subsystem}`}>
                  <code>{event.code}</code>{" "}
                  <span className="lm-subtle">
                    ({event.subsystem}) {event.message}
                  </span>
                </li>
              ))}
          </ul>
        </div>
      ) : null}

      <div className="diagnostics-panel__actions row controls">
        <button type="button" className="lm-btn" disabled={busy || loading} onClick={() => void load()}>
          Actualiser
        </button>
        <button
          type="button"
          className="lm-btn"
          disabled={busy || loading}
          onClick={() => void handlePreview()}
        >
          Aperçu du bundle
        </button>
        <button
          type="button"
          className="lm-btn lm-btn--primary"
          disabled={busy || loading}
          onClick={() => void handleSave()}
        >
          Enregistrer le ZIP
        </button>
        <button
          type="button"
          className="lm-btn"
          disabled={busy || loading}
          onClick={() => void handleCopyReport()}
        >
          Copier le rapport GitHub
        </button>
      </div>

      {preview ? (
        <div className="diagnostics-panel__preview">
          <h3>Aperçu exact du bundle</h3>
          <ul className="diagnostics-panel__files">
            {preview.files.map((file) => (
              <li key={file.name}>
                <strong>{file.name}</strong>{" "}
                <span className="lm-subtle">({formatBytes(file.sizeBytes)})</span>
              </li>
            ))}
          </ul>
          <pre className="diagnostics-panel__preview-text">{preview.previewText}</pre>
        </div>
      ) : null}

      {status ? (
        <p className="diagnostics-panel__status" role="status">
          {status}
        </p>
      ) : null}
      {error ? (
        <p className="error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
