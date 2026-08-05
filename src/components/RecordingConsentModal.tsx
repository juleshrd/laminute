interface RecordingConsentModalProps {
  onConfirm: () => void;
  onCancel: () => void;
}

export function RecordingConsentModal({ onConfirm, onCancel }: RecordingConsentModalProps) {
  return (
    <div className="modal-overlay" role="presentation" onClick={onCancel}>
      <div
        className="modal-dialog"
        role="dialog"
        aria-labelledby="recording-consent-title"
        aria-modal="true"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 id="recording-consent-title">Avertissement d&apos;enregistrement</h2>
        <p>
          L&apos;enregistrement peut capturer la voix d&apos;autres personnes présentes. Informez
          les participants et obtenez leur accord avant de commencer.
        </p>
        <div className="row controls modal-dialog__actions">
          <button type="button" onClick={onConfirm}>
            J&apos;ai informé les participants — Démarrer
          </button>
          <button type="button" className="modal-dialog__cancel" onClick={onCancel}>
            Annuler
          </button>
        </div>
      </div>
    </div>
  );
}
