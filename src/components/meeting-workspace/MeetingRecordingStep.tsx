import { formatDuration } from "../../lib/audio";

interface MeetingRecordingStepProps {
  durationSecs: number;
  onStopRecording: () => void;
}

export function MeetingRecordingStep({ durationSecs, onStopRecording }: MeetingRecordingStepProps) {
  return (
    <section className="meeting-recording-stage" aria-live="polite">
      <p className="meeting-timer" aria-label="Durée de l'enregistrement">
        {formatDuration(durationSecs)}
      </p>
      <button
        type="button"
        className="lm-btn meeting-recording-stage__stop"
        onClick={() => void onStopRecording()}
      >
        Terminer la réunion
      </button>
    </section>
  );
}
