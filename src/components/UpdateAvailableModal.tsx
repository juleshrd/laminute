import { formatUpdateProgress, type UpdateProgress } from "../lib/updater";

interface UpdateAvailableModalProps {
  currentVersion: string;
  nextVersion: string;
  notes?: string | null;
  busy: boolean;
  progress: UpdateProgress | null;
  error: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}

export function UpdateAvailableModal({
  currentVersion,
  nextVersion,
  notes,
  busy,
  progress,
  error,
  onConfirm,
  onCancel,
}: UpdateAvailableModalProps) {
  return (
    <div className="modal-overlay" role="presentation" onClick={busy ? undefined : onCancel}>
      <div
        className="modal-dialog"
        role="dialog"
        aria-labelledby="update-available-title"
        aria-modal="true"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 id="update-available-title">Mise à jour disponible</h2>
        <p>
          La version {nextVersion} est disponible (vous utilisez actuellement {currentVersion}).
        </p>
        {notes ? <p className="update-modal__notes">{notes}</p> : null}
        {busy && progress ? (
          <p className="progress-message" role="status">
            {formatUpdateProgress(progress)}
          </p>
        ) : null}
        {error ? (
          <p className="error" role="alert">
            {error}
          </p>
        ) : null}
        <div className="row controls modal-dialog__actions">
          <button type="button" onClick={onConfirm} disabled={busy}>
            Mettre à jour
          </button>
          <button type="button" className="modal-dialog__cancel" onClick={onCancel} disabled={busy}>
            Plus tard
          </button>
        </div>
      </div>
    </div>
  );
}
