interface MeetingIdleStepProps {
  canStartRecording: boolean;
  hasDevices: boolean;
  importing: boolean;
  dragOver: boolean;
  deviceName: string | null;
  devices: Array<{ id: string; name: string }>;
  selectedDeviceId: string;
  onSelectDevice: (deviceId: string) => void;
  onRequestStartRecording: () => void;
  onPickMp3: () => void;
  onDragEnter: () => void;
  onDragLeave: () => void;
}

function todayKicker(): string {
  return new Date()
    .toLocaleDateString("fr-FR", { weekday: "long", day: "numeric", month: "long" })
    .toUpperCase();
}

export function MeetingIdleStep({
  canStartRecording,
  hasDevices,
  importing,
  dragOver,
  deviceName,
  devices,
  selectedDeviceId,
  onSelectDevice,
  onRequestStartRecording,
  onPickMp3,
  onDragEnter,
  onDragLeave,
}: MeetingIdleStepProps) {
  return (
    <section className="today-view" aria-labelledby="today-hello">
      <div className="today-view__top">
        <div>
          <p className="lm-kicker">{todayKicker()}</p>
          <h2 id="today-hello">Bonjour.</h2>
          <p className="today-view__lead">
            Capturez l&apos;essentiel. La Minute s&apos;occupe du reste.
          </p>
        </div>
        <span className="lm-badge-local">⌁ Stockage local</span>
      </div>

      <div className="record-card">
        <button
          type="button"
          className="record-card__btn"
          aria-label="Démarrer l'enregistrement"
          onClick={onRequestStartRecording}
          disabled={!canStartRecording}
        >
          ●
        </button>
        <h3>Nouvelle réunion</h3>
        <p>Un clic. Aucun formulaire.</p>

        <div className="record-card__source">
          {!hasDevices ? (
            <span className="warning">Aucun périphérique d&apos;entrée audio détecté</span>
          ) : (
            <>
              <span className="record-card__source-dot" aria-hidden="true" />
              <label className="record-card__source-label">
                <span className="visually-hidden">Source audio</span>
                <select
                  value={selectedDeviceId}
                  onChange={(event) => onSelectDevice(event.target.value)}
                  aria-label="Changer la source audio"
                >
                  {devices.map((device) => (
                    <option key={device.id} value={device.id}>
                      {device.name}
                    </option>
                  ))}
                </select>
              </label>
              {deviceName ? <span className="visually-hidden">{deviceName}</span> : null}
            </>
          )}
        </div>
      </div>

      <div className="today-view__or" aria-hidden="true">
        <span>ou</span>
      </div>

      <button
        type="button"
        className={`import-card${dragOver ? " is-dragover" : ""}`}
        onClick={() => void onPickMp3()}
        disabled={importing}
        onDragEnter={onDragEnter}
        onDragLeave={onDragLeave}
        onDragOver={(event) => {
          event.preventDefault();
          onDragEnter();
        }}
      >
        <span className="import-card__icon" aria-hidden="true">
          ↥
        </span>
        <span className="import-card__copy">
          <b>{importing ? "Import en cours…" : "Importer un enregistrement"}</b>
          <span>MP3, M4A ou WAV — traité de la même façon</span>
        </span>
        <span className="import-card__chevron" aria-hidden="true">
          ›
        </span>
      </button>
    </section>
  );
}
