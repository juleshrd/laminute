import { ClockMark } from "../ClockMark";

interface MeetingIdleStepProps {
  canStartRecording: boolean;
  hasDevices: boolean;
  importing: boolean;
  dragOver: boolean;
  onRequestStartRecording: () => void;
  onPickMp3: () => void;
  onDragEnter: () => void;
  onDragLeave: () => void;
}

export function MeetingIdleStep({
  canStartRecording,
  hasDevices,
  importing,
  dragOver,
  onRequestStartRecording,
  onPickMp3,
  onDragEnter,
  onDragLeave,
}: MeetingIdleStepProps) {
  return (
    <section className="meeting-hero" aria-labelledby="meeting-hero-title">
      <h2 id="meeting-hero-title">Prêt à enregistrer ?</h2>

      <button
        type="button"
        className="meeting-logo-cta"
        aria-label="Démarrer l'enregistrement"
        onClick={onRequestStartRecording}
        disabled={!canStartRecording}
      >
        <ClockMark className="meeting-logo-cta__mark" />
      </button>

      {!hasDevices ? (
        <p className="warning meeting-hero__hint">Aucun périphérique d&apos;entrée détecté.</p>
      ) : (
        <p className="meeting-hero__hint lm-subtle">
          Cliquez sur le logo pour démarrer la réunion.
        </p>
      )}

      <section
        className={`meeting-hero__import drop-zone${dragOver ? " drop-zone-active" : ""}`}
        onDragEnter={onDragEnter}
        onDragLeave={onDragLeave}
        onDragOver={(event) => {
          event.preventDefault();
          onDragEnter();
        }}
      >
        <p className="drop-zone-hint">Ou importez un fichier MP3</p>
        <div className="row controls">
          <button type="button" onClick={() => void onPickMp3()} disabled={importing}>
            {importing ? "Import en cours…" : "Choisir un fichier MP3"}
          </button>
        </div>
        <p className="drop-zone-constraints">
          MP3 · 100 Mo max (limite transcription cloud) · 1 s–4 h
        </p>
      </section>
    </section>
  );
}
