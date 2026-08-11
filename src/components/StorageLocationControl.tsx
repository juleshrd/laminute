import { useState } from "react";

import {
  applyLocalStorageChange,
  chooseStorageParent,
  formatBytes,
  prepareLocalStorageChange,
  type LocalStorageInfo,
  type StorageChangePreview,
} from "../lib/privacy";

interface StorageLocationControlProps {
  storage: LocalStorageInfo;
  onStorageChanged: () => void | Promise<void>;
  compact?: boolean;
}

export function StorageLocationControl({
  storage,
  onStorageChanged,
  compact = false,
}: StorageLocationControlProps) {
  const [preview, setPreview] = useState<StorageChangePreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  async function prepare(useDefault: boolean) {
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const selectedParent = useDefault ? null : await chooseStorageParent();
      if (!useDefault && selectedParent == null) {
        return;
      }
      setPreview(await prepareLocalStorageChange(selectedParent, useDefault));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Impossible de valider ce dossier.");
    } finally {
      setBusy(false);
    }
  }

  async function applyChange() {
    if (!preview) return;
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const result = await applyLocalStorageChange(preview.token);
      setPreview(null);
      await onStorageChanged();
      if (result.sourceCleanupWarning) {
        setError(result.sourceCleanupWarning);
      } else {
        setStatus(`Stockage déplacé vers ${result.rootDir}.`);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "La migration du stockage a échoué.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className={`storage-location${compact ? " storage-location--compact" : ""}`}>
      <div className="storage-location__current">
        <div>
          <strong>Dossier de stockage</strong>
          <code title={storage.rootDir}>{storage.rootDir}</code>
        </div>
        <span className="storage-location__badge">
          {storage.isCustom ? "Personnalisé" : "Par défaut"}
        </span>
      </div>

      <p className="lm-subtle storage-location__scope">
        Ce dossier contient la base locale — réunions, transcriptions et résumés — ainsi que les
        imports et enregistrements audio. Un petit fichier de configuration reste dans le dossier
        système de l’application pour retrouver cet emplacement au démarrage.
      </p>

      <div className="storage-location__actions">
        <button
          type="button"
          className="lm-btn"
          disabled={busy}
          onClick={() => void prepare(false)}
        >
          Choisir un autre emplacement
        </button>
        {storage.isCustom ? (
          <button
            type="button"
            className="lm-btn"
            disabled={busy}
            onClick={() => void prepare(true)}
          >
            Revenir au dossier par défaut
          </button>
        ) : null}
      </div>

      {preview ? (
        <div
          className="storage-location__preview"
          role="dialog"
          aria-labelledby="storage-preview-title"
        >
          <h4 id="storage-preview-title">Confirmer le déplacement</h4>
          <p>
            <strong>{formatBytes(preview.dataBytes)}</strong> seront déplacés vers :
          </p>
          <code title={preview.destinationPath}>{preview.destinationPath}</code>
          <p className="lm-subtle">
            Espace disponible avant migration : {formatBytes(preview.availableBytes)}. L’ancien
            emplacement sera supprimé seulement après vérification de la copie et de la base SQLite.
          </p>
          <div className="storage-location__actions">
            <button
              type="button"
              className="lm-btn lm-btn-primary"
              disabled={busy}
              onClick={() => void applyChange()}
            >
              {busy ? "Migration…" : "Déplacer les données"}
            </button>
            <button
              type="button"
              className="lm-btn"
              disabled={busy}
              onClick={() => setPreview(null)}
            >
              Annuler
            </button>
          </div>
        </div>
      ) : null}

      {status ? (
        <p className="privacy-settings__status" role="status">
          {status}
        </p>
      ) : null}
      {error ? (
        <p className="privacy-settings__error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
