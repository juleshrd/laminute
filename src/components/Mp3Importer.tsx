import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { type MeetingDetail, formatAudioError, formatDuration } from "../lib/audio";

function isMp3Path(path: string): boolean {
  return path.toLowerCase().endsWith(".mp3");
}

function durationFromMeeting(detail: MeetingDetail): number | null {
  const durationMs = detail.audioFiles[0]?.durationMs;
  if (durationMs === null || durationMs === undefined) {
    return null;
  }
  return Math.round(durationMs / 1000);
}

export function Mp3Importer() {
  const [importing, setImporting] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastImport, setLastImport] = useState<MeetingDetail | null>(null);

  const importFile = useCallback(async (sourcePath: string) => {
    if (!isMp3Path(sourcePath)) {
      setError("Seuls les fichiers MP3 sont acceptés.");
      return;
    }

    setImporting(true);
    setError(null);

    try {
      const detail = await invoke<MeetingDetail>("import_mp3_meeting", {
        sourcePath,
      });
      setLastImport(detail);
    } catch (err) {
      setLastImport(null);
      setError(formatAudioError(err));
    } finally {
      setImporting(false);
      setDragOver(false);
    }
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    void listen<{ paths: string[] }>("tauri://drag-drop", (event) => {
      if (!active || importing) {
        return;
      }

      const mp3Path = event.payload.paths.find((path) => isMp3Path(path));
      if (!mp3Path) {
        setError("Déposez un fichier MP3 valide.");
        setDragOver(false);
        return;
      }

      void importFile(mp3Path);
    }).then((dispose) => {
      if (!active) {
        dispose();
        return;
      }
      unlisten = dispose;
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [importFile, importing]);

  async function handlePickFile() {
    setError(null);

    const selected = await open({
      multiple: false,
      filters: [{ name: "MP3", extensions: ["mp3"] }],
    });

    if (selected === null) {
      return;
    }

    const sourcePath = Array.isArray(selected) ? selected[0] : selected;
    if (!sourcePath) {
      return;
    }

    await importFile(sourcePath);
  }

  return (
    <div className="audio-test">
      <h2>Import MP3</h2>

      <section
        className={`panel drop-zone${dragOver ? " drop-zone-active" : ""}`}
        onDragEnter={() => setDragOver(true)}
        onDragLeave={() => setDragOver(false)}
        onDragOver={(event) => {
          event.preventDefault();
          setDragOver(true);
        }}
      >
        <h3>Fichier audio</h3>
        <p className="drop-zone-hint">
          Glissez-déposez un fichier MP3 ici ou sélectionnez-le depuis votre ordinateur.
        </p>
        <div className="row controls">
          <button type="button" onClick={() => void handlePickFile()} disabled={importing}>
            {importing ? "Import en cours…" : "Choisir un fichier MP3"}
          </button>
        </div>
        <p className="drop-zone-constraints">MP3 uniquement · 500 Mo max · entre 1 s et 4 h</p>
      </section>

      {lastImport && (
        <section className="panel success-panel">
          <h3>Réunion créée</h3>
          <dl className="status-grid">
            <div>
              <dt>Titre</dt>
              <dd>{lastImport.title}</dd>
            </div>
            <div>
              <dt>Statut</dt>
              <dd>Prête à traiter</dd>
            </div>
            <div>
              <dt>Durée</dt>
              <dd>{formatDuration(durationFromMeeting(lastImport))}</dd>
            </div>
            <div>
              <dt>Identifiant</dt>
              <dd className="mono">{lastImport.id}</dd>
            </div>
            <div>
              <dt>Fichier local</dt>
              <dd className="mono">{lastImport.audioFiles[0]?.filePath ?? "—"}</dd>
            </div>
          </dl>
        </section>
      )}

      {error && <p className="error">{error}</p>}
    </div>
  );
}
