import { useCallback, useEffect, useState } from "react";

import { privacySettingsIntro } from "../content/privacyNotices";
import { PRIVACY_POLICY_SHORT } from "../content/privacyPolicyShort";
import {
  deleteAllLocalData,
  formatBytes,
  getLocalStorageInfo,
  type LocalStorageInfo,
} from "../lib/privacy";
import { setOnboardingDone } from "../lib/preferences";

export function PrivacySettings() {
  const [storage, setStorage] = useState<LocalStorageInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [confirmStep, setConfirmStep] = useState<"idle" | "prompt" | "typing">("idle");
  const [confirmText, setConfirmText] = useState("");
  const [busy, setBusy] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const info = await getLocalStorageInfo();
      setStorage(info);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Chargement impossible.");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  function startWipe() {
    if (
      !window.confirm(
        "Supprimer toutes les réunions, fichiers audio, réglages et clés API locales ? Cette action est irréversible.",
      )
    ) {
      return;
    }
    setConfirmStep("typing");
    setConfirmText("");
    setStatusMessage(null);
  }

  async function executeWipe() {
    if (confirmText !== "SUPPRIMER") {
      return;
    }

    setBusy(true);
    setError(null);
    try {
      await deleteAllLocalData();
      setOnboardingDone(false);
      setStatusMessage("Toutes les données locales ont été supprimées.");
      setConfirmStep("idle");
      setConfirmText("");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Suppression impossible.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="privacy-settings" aria-labelledby="privacy-settings-title">
      <header className="privacy-settings__header">
        <h2 id="privacy-settings-title">Confidentialité et données locales</h2>
        <p>{privacySettingsIntro()}</p>
      </header>

      {loading && <p className="privacy-settings__loading">Chargement…</p>}

      {storage && (
        <dl className="status-grid privacy-settings__storage">
          <div>
            <dt>Réunions enregistrées</dt>
            <dd>{storage.meetingsCount}</dd>
          </div>
          <div>
            <dt>Base de données</dt>
            <dd className="mono">{storage.dbPath}</dd>
          </div>
          <div>
            <dt>Imports</dt>
            <dd className="mono">
              {storage.importsDir}
              {storage.importsBytes != null && (
                <span className="privacy-settings__size">
                  {" "}
                  ({formatBytes(storage.importsBytes)})
                </span>
              )}
            </dd>
          </div>
          <div>
            <dt>Enregistrements</dt>
            <dd className="mono">
              {storage.recordingsDir}
              {storage.recordingsBytes != null && (
                <span className="privacy-settings__size">
                  {" "}
                  ({formatBytes(storage.recordingsBytes)})
                </span>
              )}
            </dd>
          </div>
        </dl>
      )}

      <details className="privacy-settings__policy">
        <summary>Résumé de la politique de confidentialité</summary>
        <p className="privacy-settings__policy-text">{PRIVACY_POLICY_SHORT}</p>
      </details>

      {confirmStep === "typing" ? (
        <div className="privacy-settings__wipe-confirm">
          <label htmlFor="wipe-confirm-input">
            Tapez <strong>SUPPRIMER</strong> pour confirmer l&apos;effacement complet
          </label>
          <input
            id="wipe-confirm-input"
            type="text"
            value={confirmText}
            disabled={busy}
            onChange={(event) => setConfirmText(event.target.value)}
            autoComplete="off"
          />
          <div className="row controls">
            <button
              type="button"
              className="privacy-settings__danger"
              disabled={busy || confirmText !== "SUPPRIMER"}
              onClick={() => void executeWipe()}
            >
              Effacer toutes les données
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => {
                setConfirmStep("idle");
                setConfirmText("");
              }}
            >
              Annuler
            </button>
          </div>
        </div>
      ) : (
        <div className="privacy-settings__actions">
          <button
            type="button"
            className="privacy-settings__danger"
            disabled={busy || loading}
            onClick={startWipe}
          >
            Effacer toutes les données locales
          </button>
        </div>
      )}

      {statusMessage && (
        <p className="privacy-settings__status" role="status">
          {statusMessage}
        </p>
      )}

      {error && (
        <p className="privacy-settings__error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
