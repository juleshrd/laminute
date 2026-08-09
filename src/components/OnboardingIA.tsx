import { useEffect, useState } from "react";

import { onboardingProviderDescription, onboardingWelcomeLead } from "../content/privacyNotices";
import { getAiSettings, listAiProviders, setSelectedProvider } from "../lib/ai/api";
import type { ProviderInfo } from "../lib/ai/types";
import {
  prepareFirstRun,
  saveFirstRunStoragePreference,
  type FirstRunStatus,
} from "../lib/firstRun";
import { BrandMark } from "./LmShell";
import { ProviderCredentialsForm } from "./ProviderCredentialsForm";
import { ProviderLogo } from "./ProviderLogo";
import { ToggleSwitch } from "./ToggleSwitch";
import "./AiProviderSettings.css";

type OnboardingStep = "welcome" | "storage" | "choose" | "config" | "ready";

interface OnboardingIAProps {
  onComplete: () => void;
  onSkip: () => void;
}

function providerDescription(provider: ProviderInfo): string {
  return onboardingProviderDescription(provider.id, provider.capabilities);
}

const STEPS: OnboardingStep[] = ["welcome", "storage", "choose", "config", "ready"];

function stepIndex(step: OnboardingStep): number {
  return STEPS.indexOf(step);
}

function compactPath(path: string): string {
  const homeMatch = path.match(/^(\/Users\/[^/]+|\/home\/[^/]+)(\/.*)?$/);
  return homeMatch ? `~${homeMatch[2] ?? ""}` : path;
}

export function OnboardingIA({ onComplete, onSkip }: OnboardingIAProps) {
  const [step, setStep] = useState<OnboardingStep>("welcome");
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedId, setSelectedId] = useState("mistral");
  const [hasStoredKey, setHasStoredKey] = useState(false);
  const [ollamaBaseUrl, setOllamaBaseUrl] = useState("http://127.0.0.1:11434");
  const [ollamaAllowRemote, setOllamaAllowRemote] = useState(false);
  const [setupStatus, setSetupStatus] = useState<FirstRunStatus | null>(null);
  const [keepAudioFiles, setKeepAudioFiles] = useState(true);
  const [loadingSetup, setLoadingSetup] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [credKey, setCredKey] = useState(0);

  async function loadSetup() {
    setLoadingSetup(true);
    setError(null);
    try {
      const status = await prepareFirstRun();
      setSetupStatus(status);
      setKeepAudioFiles(status.keepAudioFiles);
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : "Impossible de préparer les dossiers locaux. Vérifiez les autorisations du disque.",
      );
    } finally {
      setLoadingSetup(false);
    }
  }

  useEffect(() => {
    void loadSetup();
    void (async () => {
      try {
        const [list, settings] = await Promise.all([listAiProviders(), getAiSettings()]);
        setProviders(list);
        setSelectedId(settings.selectedProviderId ?? "mistral");
        setHasStoredKey(settings.hasApiKey);
        setOllamaBaseUrl(settings.ollamaBaseUrl ?? "http://127.0.0.1:11434");
        setOllamaAllowRemote(settings.ollamaAllowRemote);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Chargement de la configuration impossible.");
      }
    })();
  }, []);

  const ordered = [...providers].sort((a, b) => {
    if (a.id === "mistral") return -1;
    if (b.id === "mistral") return 1;
    return a.displayName.localeCompare(b.displayName, "fr");
  });

  const selectedProvider = providers.find((provider) => provider.id === selectedId);
  const isLocal = selectedProvider?.capabilities.local ?? false;
  const activeStep = stepIndex(step);

  const asideCopy: Record<OnboardingStep, { title: string; body: string }> = {
    welcome: {
      title: "On commence.",
      body: "Quelques réglages suffisent pour que votre première réunion fonctionne immédiatement.",
    },
    storage: {
      title: "Vos données, au clair.",
      body: "La Minute prépare ses dossiers privés et vous laisse choisir combien de temps garder les audios.",
    },
    choose: {
      title: "Choisissez l’IA.",
      body: "Mistral est recommandé. Vous pourrez changer de fournisseur plus tard.",
    },
    config: {
      title: "Connectez le service.",
      body: "La clé reste dans le trousseau macOS. Sans clé, l’app reste utilisable en mode limité.",
    },
    ready: {
      title: "Tout est prêt.",
      body: "Les réglages essentiels sont appliqués. Vous pouvez lancer ou importer une réunion.",
    },
  };

  async function finishStorageStep() {
    setBusy(true);
    setError(null);
    try {
      await saveFirstRunStoragePreference(keepAudioFiles);
      setStep("choose");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Enregistrement de la préférence impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function goToConfig() {
    setBusy(true);
    setError(null);
    try {
      const settings = await setSelectedProvider(selectedId);
      setHasStoredKey(settings.hasApiKey);
      setOllamaBaseUrl(settings.ollamaBaseUrl ?? "http://127.0.0.1:11434");
      setOllamaAllowRemote(settings.ollamaAllowRemote);
      setCredKey((key) => key + 1);
      setStep("config");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sélection impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function reviewSetup() {
    setBusy(true);
    setError(null);
    try {
      await setSelectedProvider(selectedId);
      setStep("ready");
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
          <p className="lm-onboarding-kicker">Configuration · {activeStep + 1}/5</p>
          <h2 id="onboarding-title" className="lm-welcome-title">
            {asideCopy[step].title}
          </h2>
          <p className="lm-subtle">{asideCopy[step].body}</p>
        </div>
        <div className="lm-steps" aria-label={`Étape ${activeStep + 1} sur 5`}>
          {STEPS.map((item, index) => (
            <span
              key={item}
              className={`lm-step${index === activeStep ? " is-active" : ""}${index < activeStep ? " is-done" : ""}`}
            />
          ))}
        </div>
      </aside>

      <main className="lm-choose">
        {step === "welcome" ? (
          <>
            <p className="lm-eyebrow">Premier lancement</p>
            <h2>Bienvenue</h2>
            <p className="lm-subtle lm-onboarding-lead">{onboardingWelcomeLead()}</p>
            <ul className="lm-onboarding-benefits">
              <li>Dossiers locaux préparés et vérifiés</li>
              <li>Microphone demandé seulement au premier enregistrement</li>
              <li>Fournisseur IA modifiable à tout moment</li>
            </ul>
            {ordered.length > 0 ? (
              <ul className="lm-provider-logo-row" aria-label="Fournisseurs IA disponibles">
                {ordered.map((provider) => (
                  <li key={provider.id} className="lm-provider-logo-row__item">
                    <ProviderLogo
                      providerId={provider.id}
                      displayName={provider.displayName}
                      size="sm"
                    />
                    <span>{provider.displayName}</span>
                  </li>
                ))}
              </ul>
            ) : null}
            <div className="lm-row lm-onboarding-actions">
              <button type="button" className="lm-btn" disabled={busy} onClick={onSkip}>
                Découvrir sans configurer
              </button>
              <button
                type="button"
                className="lm-btn lm-btn-primary"
                disabled={busy}
                onClick={() => setStep("storage")}
              >
                Configurer l’app
              </button>
            </div>
            <p className="lm-onboarding-footnote">
              Si vous passez cette étape, elle sera reproposée au prochain lancement.
            </p>
          </>
        ) : null}

        {step === "storage" ? (
          <>
            <p className="lm-eyebrow">Stockage et microphone</p>
            <h2>Votre espace est préparé</h2>
            <p className="lm-subtle lm-onboarding-lead">
              Les réunions restent dans le dossier privé de La Minute. Les exports PDF ou Markdown
              vous demanderont toujours où enregistrer le fichier.
            </p>

            {loadingSetup ? <p className="lm-setup-loading">Vérification des dossiers…</p> : null}

            {setupStatus ? (
              <div className="lm-setup-card">
                <div className="lm-setup-row">
                  <span className="lm-setup-icon" aria-hidden="true">
                    ✓
                  </span>
                  <div>
                    <strong>Données et réunions</strong>
                    <code title={setupStatus.storage.dbPath}>
                      {compactPath(setupStatus.storage.dbPath)}
                    </code>
                  </div>
                </div>
                <div className="lm-setup-row">
                  <span className="lm-setup-icon" aria-hidden="true">
                    ✓
                  </span>
                  <div>
                    <strong>Enregistrements audio</strong>
                    <code title={setupStatus.storage.recordingsDir}>
                      {compactPath(setupStatus.storage.recordingsDir)}
                    </code>
                  </div>
                </div>
                <div className="lm-setup-row">
                  <span className="lm-setup-icon is-muted" aria-hidden="true">
                    ○
                  </span>
                  <div>
                    <strong>Microphone</strong>
                    <span>Autorisation demandée au premier enregistrement</span>
                  </div>
                </div>
              </div>
            ) : null}

            <div className="lm-setup-preference">
              <div>
                <strong>Conserver les audios après traitement</strong>
                <p className="lm-subtle">
                  Désactivez pour supprimer automatiquement l’audio après transcription.
                </p>
              </div>
              <ToggleSwitch
                checked={keepAudioFiles}
                disabled={busy || loadingSetup || !setupStatus}
                aria-label="Conserver les audios après traitement"
                onChange={setKeepAudioFiles}
              />
            </div>

            {error ? (
              <div className="lm-setup-error" role="alert">
                <p>{error}</p>
                <button
                  type="button"
                  className="lm-btn"
                  disabled={loadingSetup}
                  onClick={() => void loadSetup()}
                >
                  Réessayer
                </button>
              </div>
            ) : null}

            <div className="lm-row lm-onboarding-actions">
              <button
                type="button"
                className="lm-btn"
                disabled={busy}
                onClick={() => setStep("welcome")}
              >
                Retour
              </button>
              <button
                type="button"
                className="lm-btn lm-btn-primary"
                disabled={busy || loadingSetup || !setupStatus}
                onClick={() => void finishStorageStep()}
              >
                Continuer
              </button>
            </div>
          </>
        ) : null}

        {step === "choose" ? (
          <>
            <p className="lm-eyebrow">Intelligence artificielle</p>
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
                      <ProviderLogo
                        providerId={provider.id}
                        displayName={provider.displayName}
                        size={featured ? "lg" : "md"}
                      />
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
              <button
                type="button"
                className="lm-btn"
                disabled={busy}
                onClick={() => setStep("storage")}
              >
                Retour
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
            <p className="lm-eyebrow">Connexion</p>
            <h2 className="lm-config-title">
              {selectedProvider ? (
                <ProviderLogo
                  providerId={selectedProvider.id}
                  displayName={selectedProvider.displayName}
                  size="md"
                />
              ) : null}
              <span>Configurer {selectedProvider?.displayName ?? "le fournisseur"}</span>
            </h2>
            <p className="lm-subtle lm-onboarding-lead">
              {isLocal
                ? "Testez la connexion à votre serveur local avant de continuer."
                : "Enregistrez puis validez votre clé. Elle ne quitte pas le trousseau sécurisé du système."}
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

            {!isLocal && !hasStoredKey ? (
              <p className="lm-limited-note">
                Sans clé, vous pourrez importer un MP3 et coller du texte, mais la transcription et
                le compte-rendu IA resteront indisponibles.
              </p>
            ) : null}

            {error ? (
              <p className="error" role="alert">
                {error}
              </p>
            ) : null}

            <div className="lm-row lm-onboarding-actions">
              <button
                type="button"
                className="lm-btn"
                disabled={busy}
                onClick={() => setStep("choose")}
              >
                Retour
              </button>
              <button
                type="button"
                className="lm-btn lm-btn-primary"
                disabled={busy || !selectedId}
                onClick={() => void reviewSetup()}
              >
                {!isLocal && !hasStoredKey ? "Continuer en mode limité" : "Continuer"}
              </button>
            </div>
          </>
        ) : null}

        {step === "ready" ? (
          <>
            <p className="lm-eyebrow">Configuration terminée</p>
            <h2>La Minute est prête</h2>
            <div className="lm-ready-list">
              <div>
                <span aria-hidden="true">✓</span>
                <p>
                  <strong>Stockage local</strong>
                  <br />
                  Dossiers créés et accessibles
                </p>
              </div>
              <div>
                <span aria-hidden="true">○</span>
                <p>
                  <strong>Enregistrement</strong>
                  <br />
                  Micro demandé à la première utilisation
                </p>
              </div>
              <div>
                <span aria-hidden="true">{hasStoredKey || isLocal ? "✓" : "—"}</span>
                <p>
                  <strong>IA</strong>
                  <br />
                  {hasStoredKey || isLocal
                    ? selectedProvider?.displayName
                    : "Mode limité — à terminer plus tard"}
                </p>
              </div>
            </div>
            <p className="lm-subtle lm-onboarding-lead">
              Tous ces choix restent modifiables dans Réglages.
            </p>
            <div className="lm-row lm-onboarding-actions">
              <button
                type="button"
                className="lm-btn"
                disabled={busy}
                onClick={() => setStep("config")}
              >
                Retour
              </button>
              <button
                type="button"
                className="lm-btn lm-btn-primary"
                disabled={busy}
                onClick={onComplete}
              >
                Créer ma première réunion
              </button>
            </div>
          </>
        ) : null}
      </main>
    </section>
  );
}
