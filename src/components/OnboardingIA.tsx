import { useEffect, useState } from "react";

import { getAiSettings, listAiProviders, setSelectedProvider } from "../lib/ai/api";
import type { ProviderInfo } from "../lib/ai/types";
import { BrandMark } from "./LmShell";
import { ProviderCredentialsForm } from "./ProviderCredentialsForm";
import { ProviderLogo } from "./ProviderLogo";
import "./AiProviderSettings.css";

type OnboardingStep = "welcome" | "choose" | "config";

interface OnboardingIAProps {
  onComplete: () => void;
  onSkip: () => void;
}

function providerDescription(provider: ProviderInfo): string {
  if (provider.capabilities.local) {
    return "Traitement local via Ollama.";
  }
  if (provider.capabilities.transcription) {
    return "Transcription + compte-rendu.";
  }
  return "Compte-rendu à partir d’un texte.";
}

function stepIndex(step: OnboardingStep): number {
  if (step === "welcome") return 0;
  if (step === "choose") return 1;
  return 2;
}

export function OnboardingIA({ onComplete, onSkip }: OnboardingIAProps) {
  const [step, setStep] = useState<OnboardingStep>("welcome");
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedId, setSelectedId] = useState("mistral");
  const [hasStoredKey, setHasStoredKey] = useState(false);
  const [ollamaBaseUrl, setOllamaBaseUrl] = useState("http://127.0.0.1:11434");
  const [ollamaAllowRemote, setOllamaAllowRemote] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [credKey, setCredKey] = useState(0);

  useEffect(() => {
    void (async () => {
      try {
        const [list, settings] = await Promise.all([listAiProviders(), getAiSettings()]);
        setProviders(list);
        setSelectedId(settings.selectedProviderId ?? "mistral");
        setHasStoredKey(settings.hasApiKey);
        setOllamaBaseUrl(settings.ollamaBaseUrl ?? "http://127.0.0.1:11434");
        setOllamaAllowRemote(settings.ollamaAllowRemote);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Chargement impossible.");
      }
    })();
  }, []);

  const ordered = [...providers].sort((a, b) => {
    if (a.id === "mistral") return -1;
    if (b.id === "mistral") return 1;
    return a.displayName.localeCompare(b.displayName, "fr");
  });

  const selectedProvider = providers.find((p) => p.id === selectedId);
  const isLocal = selectedProvider?.capabilities.local ?? false;
  const activeStep = stepIndex(step);

  const asideCopy =
    step === "welcome"
      ? {
          title: "On commence.",
          body: "La Minute transforme vos réunions en comptes-rendus structurés — synthèse, décisions et actions.",
        }
      : step === "choose"
        ? {
            title: "Choisissez l’IA.",
            body: "Mistral est recommandé. Vous pourrez changer de fournisseur plus tard dans les réglages.",
          }
        : {
            title: "Configurez.",
            body: "Collez votre clé API ou connectez Ollama. Vous pourrez finaliser plus tard dans les réglages.",
          };

  async function goToConfig() {
    setBusy(true);
    setError(null);
    try {
      const settings = await setSelectedProvider(selectedId);
      setHasStoredKey(settings.hasApiKey);
      setOllamaBaseUrl(settings.ollamaBaseUrl ?? "http://127.0.0.1:11434");
      setOllamaAllowRemote(settings.ollamaAllowRemote);
      setCredKey((k) => k + 1);
      setStep("config");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sélection impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function finish() {
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

  return (
    <section className="lm-onboarding" aria-labelledby="onboarding-title">
      <aside className="lm-welcome">
        <div>
          <div className="lm-nav-brand">
            <BrandMark />
            La Minute
          </div>
          <h2 id="onboarding-title" className="lm-welcome-title">
            {asideCopy.title}
          </h2>
          <p className="lm-subtle">{asideCopy.body}</p>
        </div>
        <div className="lm-steps" aria-hidden="true">
          {[0, 1, 2].map((index) => (
            <span
              key={index}
              className={`lm-step${index === activeStep ? " is-active" : ""}${index < activeStep ? " is-done" : ""}`}
            />
          ))}
        </div>
      </aside>

      <main className="lm-choose">
        {step === "welcome" ? (
          <>
            <h2>Bienvenue</h2>
            <p className="lm-subtle lm-onboarding-lead">
              Enregistrez ou importez un audio, lancez le traitement, et consultez le résultat. Tout
              reste sur votre ordinateur — avec la clé du fournisseur que vous choisissez.
            </p>
            <div className="lm-row lm-onboarding-actions">
              <button type="button" className="lm-btn" disabled={busy} onClick={onSkip}>
                Plus tard
              </button>
              <button
                type="button"
                className="lm-btn lm-btn-primary"
                disabled={busy}
                onClick={() => setStep("choose")}
              >
                Commencer
              </button>
            </div>
          </>
        ) : null}

        {step === "choose" ? (
          <>
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
                    aria-pressed={selected}
                  >
                    <span className="lm-provider-head">
                      <ProviderLogo providerId={provider.id} displayName={provider.displayName} />
                      <span className="lm-provider-title">
                        {provider.displayName}
                        {featured ? <span className="lm-recommended">Recommandé</span> : null}
                      </span>
                    </span>
                    <span className="lm-provider-desc">{providerDescription(provider)}</span>
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
                onClick={() => void goToConfig()}
              >
                Continuer
              </button>
            </div>
          </>
        ) : null}

        {step === "config" ? (
          <>
            <h2>Configurer {selectedProvider?.displayName ?? "le fournisseur"}</h2>
            <p className="lm-subtle lm-onboarding-lead">
              Optionnel pour l’instant — vous pourrez configurer plus tard dans Réglages.
            </p>

            {selectedId ? (
              <ProviderCredentialsForm
                key={`${selectedId}-${credKey}`}
                providerId={selectedId}
                isLocal={isLocal}
                hasStoredKey={hasStoredKey}
                onHasStoredKeyChange={setHasStoredKey}
                ollamaBaseUrl={ollamaBaseUrl}
                onOllamaBaseUrlChange={setOllamaBaseUrl}
                ollamaAllowRemote={ollamaAllowRemote}
                onOllamaAllowRemoteChange={setOllamaAllowRemote}
                showDelete={false}
                idPrefix="onboarding"
              />
            ) : null}

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
                onClick={() => void finish()}
              >
                Entrer dans l&apos;app
              </button>
            </div>
          </>
        ) : null}
      </main>
    </section>
  );
}
