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
      <h2 id="meeting-hero-title" className="meeting-hero__title">
        Prêt à enregistrer ?
      </h2>

      <button
        type="button"
        className="meeting-logo-cta"
        aria-label="Démarrer l'enregistrement"
        onClick={onRequestStartRecording}
        disabled={!canStartRecording}
      >
        <span className="meeting-logo-cta__halo" aria-hidden="true" />
        <span className="meeting-logo-cta__surface">
          <ClockMark className="meeting-logo-cta__mark" />
        </span>
      </button>

      {!hasDevices ? (
        <p className="meeting-hero__hint meeting-hero__hint--soft">
          Aucun micro détecté — vous pouvez quand même importer un MP3.
        </p>
      ) : (
        <p className="meeting-hero__hint lm-subtle">Cliquez sur le logo pour démarrer la réunion.</p>
      )}

      <div className="meeting-hero__divider" aria-hidden="true">
        <span>ou</span>
      </div>

      <section
        className={`meeting-hero__import drop-zone${dragOver ? " drop-zone-active" : ""}`}
        onDragEnter={onDragEnter}
        onDragLeave={onDragLeave}
        onDragOver={(event) => {
          event.preventDefault();
          onDragEnter();
        }}
      >
        <p className="drop-zone-hint">Importez un fichier MP3</p>
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
