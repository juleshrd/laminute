import { useEffect, useState } from "react";

import { getAiSettings, listAiProviders, setSelectedProvider } from "../lib/ai/api";
import type { ProviderInfo } from "../lib/ai/types";
import { BrandMark } from "./LmShell";

interface OnboardingIAProps {
  onComplete: () => void;
  onSkip: () => void;
}

export function OnboardingIA({ onComplete, onSkip }: OnboardingIAProps) {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedId, setSelectedId] = useState("mistral");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const [list, settings] = await Promise.all([listAiProviders(), getAiSettings()]);
        setProviders(list);
        setSelectedId(settings.selectedProviderId ?? "mistral");
      } catch (err) {
        setError(err instanceof Error ? err.message : "Chargement impossible.");
      }
    })();
  }, []);

  async function handleContinue() {
    setBusy(true);
    setError(null);
    try {
      await setSelectedProvider(selectedId);
      onComplete();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sélection impossible.");
    } finally {
      setBusy(false);
    }
  }

  const ordered = [...providers].sort((a, b) => {
    if (a.id === "mistral") return -1;
    if (b.id === "mistral") return 1;
    return a.displayName.localeCompare(b.displayName, "fr");
  });

  return (
    <section className="lm-onboarding" aria-labelledby="onboarding-title">
      <aside className="lm-welcome">
        <div>
          <div className="lm-nav-brand">
            <BrandMark />
            La Minute
          </div>
          <h2 id="onboarding-title" className="lm-welcome-title">
            On commence.
          </h2>
          <p className="lm-subtle">
            Choisissez votre fournisseur IA. Vous pourrez modifier ce choix plus tard dans les
            réglages.
          </p>
        </div>
        <div className="lm-steps" aria-hidden="true">
          <span className="lm-step is-active" />
          <span className="lm-step" />
          <span className="lm-step" />
        </div>
      </aside>

      <main className="lm-choose">
        <h2>Choisir l&apos;IA</h2>
        <div className="lm-provider-grid">
          {ordered.map((provider) => {
            const featured = provider.id === "mistral";
            const selected = selectedId === provider.id;
            return (
              <button
                key={provider.id}
                type="button"
                className={`lm-provider${featured ? " is-featured" : ""}${selected ? " is-selected" : ""}`}
                onClick={() => setSelectedId(provider.id)}
                disabled={busy}
              >
                <span className="lm-provider-title">
                  {provider.displayName}
                  {featured ? <span className="lm-recommended">Recommandé</span> : null}
                </span>
                <span className="lm-provider-desc">
                  {provider.capabilities.local
                    ? "Traitement local via Ollama."
                    : provider.capabilities.transcription
                      ? "Transcription + compte-rendu."
                      : "Compte-rendu à partir d’un texte."}
                </span>
              </button>
            );
          })}
        </div>

        {error ? (
          <p className="error" role="alert">
            {error}
          </p>
        ) : null}

        <div className="lm-row lm-onboarding-actions">
          <button type="button" className="lm-btn" disabled={busy} onClick={onSkip}>
            Plus tard
          </button>
          <button
            type="button"
            className="lm-btn lm-btn-primary"
            disabled={busy || !selectedId}
            onClick={() => void handleContinue()}
          >
            Continuer
          </button>
        </div>
      </main>
    </section>
  );
}
