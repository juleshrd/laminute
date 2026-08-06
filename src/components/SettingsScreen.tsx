import { useCallback, useEffect, useState } from "react";

import { getKeepAudioFiles, setKeepAudioFiles } from "../lib/audioSettings";
import {
  getReduceMotionPreference,
  setOnboardingDone,
  setReduceMotionPreference,
} from "../lib/preferences";
import { AiProviderSettings } from "./AiProviderSettings";
import { PrivacySettings } from "./PrivacySettings";
import { ToggleSwitch } from "./ToggleSwitch";

interface SettingsScreenProps {
  onReplayOnboarding: () => void;
}

export function SettingsScreen({ onReplayOnboarding }: SettingsScreenProps) {
  const [keepAudio, setKeepAudio] = useState(true);
  const [reduceMotion, setReduceMotion] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const keep = await getKeepAudioFiles();
      setKeepAudio(keep);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Impossible de charger les préférences audio.");
    }
    setReduceMotion(getReduceMotionPreference());
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleKeepAudioChange(next: boolean) {
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const saved = await setKeepAudioFiles(next);
      setKeepAudio(saved);
      setStatus(
        saved
          ? "Les fichiers audio seront conservés localement."
          : "Les fichiers audio seront supprimés après traitement.",
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "Enregistrement impossible.");
    } finally {
      setBusy(false);
    }
  }

  function handleReduceMotionChange(next: boolean) {
    setReduceMotion(next);
    setReduceMotionPreference(next);
  }

  return (
    <div className="lm-settings">
      <div className="lm-heading">
        <div>
          <h2>Réglages</h2>
          <p className="lm-subtle">IA, confidentialité et préférences locales.</p>
        </div>
      </div>

      <section className="lm-panel lm-setting">
        <div className="lm-row">
          <div>
            <h3>Audios locaux</h3>
            <p className="lm-subtle">
              Conserver les fichiers audio après transcription / compte-rendu.
            </p>
          </div>
          <ToggleSwitch
            checked={keepAudio}
            disabled={busy}
            aria-label="Conserver les audios localement"
            onChange={(next) => void handleKeepAudioChange(next)}
          />
        </div>
      </section>

      <section className="lm-panel lm-setting">
        <div className="lm-row">
          <div>
            <h3>Réduire les animations</h3>
            <p className="lm-subtle">Transitions plus courtes ou désactivées.</p>
          </div>
          <ToggleSwitch
            checked={reduceMotion}
            aria-label="Réduire les animations"
            onChange={handleReduceMotionChange}
          />
        </div>
      </section>

      <section className="lm-panel lm-setting">
        <div className="lm-row">
          <div>
            <h3>Onboarding IA</h3>
            <p className="lm-subtle">Rejouer le choix du fournisseur au prochain affichage.</p>
          </div>
          <button
            type="button"
            className="lm-btn"
            onClick={() => {
              setOnboardingDone(false);
              onReplayOnboarding();
            }}
          >
            Relancer
          </button>
        </div>
      </section>

      {status ? (
        <p className="privacy-settings__status" role="status">
          {status}
        </p>
      ) : null}
      {error ? (
        <p className="error" role="alert">
          {error}
        </p>
      ) : null}

      <section className="lm-panel lm-setting">
        <AiProviderSettings />
      </section>

      <section className="lm-panel lm-setting">
        <PrivacySettings />
      </section>
    </div>
  );
}
