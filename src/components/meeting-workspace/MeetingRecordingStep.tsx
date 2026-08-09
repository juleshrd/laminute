import { formatDuration } from "../../lib/audio";

interface MeetingRecordingStepProps {
  durationSecs: number;
  deviceName?: string | null;
  onStopRecording: () => void;
}

export function MeetingRecordingStep({
  durationSecs,
  deviceName = null,
  onStopRecording,
}: MeetingRecordingStepProps) {
  return (
    <section className="today-view today-view--recording" aria-live="polite">
      <div className="today-view__top">
        <div>
          <p className="lm-kicker">ENREGISTREMENT</p>
          <h2>Je vous écoute.</h2>
          <p className="today-view__lead">
            L&apos;état du micro reste visible partout dans l&apos;app.
          </p>
        </div>
        <span className="lm-badge-local">⌁ Stockage local</span>
      </div>

      <div className="record-card is-recording">
        <button
          type="button"
          className="record-card__btn record-card__btn--stop"
          aria-label="Arrêter l'enregistrement"
          onClick={() => void onStopRecording()}
        >
          ■
        </button>
        <h3 aria-label="Durée de l'enregistrement">{formatDuration(durationSecs)}</h3>
        <p>
          Enregistrement en cours
          {deviceName ? ` · ${deviceName}` : ""}
        </p>
        <div className="record-card__source">
          <span className="record-card__source-dot is-live" aria-hidden="true" />
          <b>{deviceName ?? "Micro"}</b>
        </div>
      </div>

      <button
        type="button"
        className="lm-btn lm-btn-primary record-card__finish"
        onClick={() => void onStopRecording()}
      >
        Terminer la réunion
      </button>
    </section>
  );
}
