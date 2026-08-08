import { useCallback, useEffect, useState } from "react";

import {
  getAiSettings,
  listAiProviders,
  setModelPreferences,
  setSelectedProvider,
} from "../lib/ai/api";
import type { AiSettings, ProviderInfo } from "../lib/ai/types";
import { formatCapabilities } from "../lib/ai/formatCapabilities";
import { DataProcessingNotice } from "./DataProcessingNotice";
import { ProviderCredentialsForm } from "./ProviderCredentialsForm";
import "./AiProviderSettings.css";

function applySettings(
  settings: AiSettings,
  setSelectedProviderId: (id: string) => void,
  setHasStoredKey: (v: boolean) => void,
  setOllamaBaseUrlState: (v: string) => void,
  setOllamaAllowRemote: (v: boolean) => void,
  setDiarizationEnabled: (v: boolean) => void,
  setTranscriptionModel: (v: string) => void,
  setSummaryModel: (v: string) => void,
  setAiSettings: (s: AiSettings) => void,
) {
  setSelectedProviderId(settings.selectedProviderId ?? "");
  setHasStoredKey(settings.hasApiKey);
  setOllamaBaseUrlState(settings.ollamaBaseUrl ?? "http://127.0.0.1:11434");
  setOllamaAllowRemote(settings.ollamaAllowRemote);
  setDiarizationEnabled(settings.diarizationEnabled);
  setTranscriptionModel(settings.transcriptionModel ?? "");
  setSummaryModel(settings.summaryModel ?? "");
  setAiSettings(settings);
}

export function AiProviderSettings() {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedProviderId, setSelectedProviderId] = useState("");
  const [ollamaBaseUrl, setOllamaBaseUrlState] = useState("http://127.0.0.1:11434");
  const [ollamaAllowRemote, setOllamaAllowRemote] = useState(false);
  const [hasStoredKey, setHasStoredKey] = useState(false);
  const [aiSettings, setAiSettings] = useState<AiSettings | null>(null);
  const [transcriptionModel, setTranscriptionModel] = useState("");
  const [summaryModel, setSummaryModel] = useState("");
  const [diarizationEnabled, setDiarizationEnabled] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [credKey, setCredKey] = useState(0);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [providerList, settings] = await Promise.all([listAiProviders(), getAiSettings()]);
      setProviders(providerList);
      const initialId = settings.selectedProviderId ?? providerList[0]?.id ?? "";
      applySettings(
        { ...settings, selectedProviderId: initialId || settings.selectedProviderId },
        setSelectedProviderId,
        setHasStoredKey,
        setOllamaBaseUrlState,
        setOllamaAllowRemote,
        setDiarizationEnabled,
        setTranscriptionModel,
        setSummaryModel,
        setAiSettings,
      );
      if (!settings.selectedProviderId && initialId) {
        setSelectedProviderId(initialId);
      }
      setCredKey((k) => k + 1);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Chargement impossible.");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const selectedProvider = providers.find((p) => p.id === selectedProviderId);
  const isLocalProvider = selectedProvider?.capabilities.local ?? false;
  const hasTranscription = selectedProvider?.capabilities.transcription ?? false;
  const hasDiarization = selectedProvider?.capabilities.diarization ?? false;
  const transcriptionModels = aiSettings?.transcriptionModels ?? [];
  const summaryModels = aiSettings?.summaryModels ?? [];

  async function handleProviderChange(nextId: string) {
    setBusy(true);
    setError(null);
    try {
      const settings = await setSelectedProvider(nextId);
      applySettings(
        settings,
        setSelectedProviderId,
        setHasStoredKey,
        setOllamaBaseUrlState,
        setOllamaAllowRemote,
        setDiarizationEnabled,
        setTranscriptionModel,
        setSummaryModel,
        setAiSettings,
      );
      setCredKey((k) => k + 1);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sélection impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function persistModelPrefs(partial: {
    transcriptionModel?: string;
    summaryModel?: string;
    diarizationEnabled?: boolean;
  }) {
    if (!selectedProviderId) return;
    setBusy(true);
    setError(null);
    try {
      const settings = await setModelPreferences({
        providerId: selectedProviderId,
        transcriptionModel: partial.transcriptionModel,
        summaryModel: partial.summaryModel,
        diarizationEnabled: partial.diarizationEnabled,
      });
      applySettings(
        settings,
        setSelectedProviderId,
        setHasStoredKey,
        setOllamaBaseUrlState,
        setOllamaAllowRemote,
        setDiarizationEnabled,
        setTranscriptionModel,
        setSummaryModel,
        setAiSettings,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "Enregistrement impossible.");
    } finally {
      setBusy(false);
    }
  }

  if (loading) {
    return <p className="ai-settings__loading">Chargement des fournisseurs…</p>;
  }

  if (providers.length === 0) {
    return <p className="ai-settings__empty">Aucun fournisseur IA disponible.</p>;
  }

  return (
    <section className="ai-settings" aria-labelledby="ai-settings-title">
      <header className="ai-settings__header">
        <h2 id="ai-settings-title">Fournisseurs IA (BYOK)</h2>
        <p>
          Sélectionnez un fournisseur et enregistrez votre clé API. Choisissez ensuite le modèle
          audio pour la transcription et le modèle LLM pour le compte-rendu. Les secrets sont
          stockés dans le trousseau système, jamais en clair dans l&apos;application.
        </p>
      </header>

      <div className="ai-settings__field">
        <label htmlFor="provider-select">Fournisseur</label>
        <select
          id="provider-select"
          value={selectedProviderId}
          disabled={busy}
          onChange={(event) => void handleProviderChange(event.target.value)}
        >
          {providers.map((provider) => (
            <option key={provider.id} value={provider.id}>
              {provider.displayName}
            </option>
          ))}
        </select>
      </div>

      {selectedProvider ? (
        <>
          <p className="ai-settings__capabilities">
            Capacités : {formatCapabilities(selectedProvider)}
          </p>
          {!hasTranscription ? (
            <p className="ai-settings__note" role="note">
              Ce fournisseur ne prend pas en charge la transcription audio. Vous pourrez générer un
              compte-rendu à partir d&apos;un texte collé.
            </p>
          ) : null}
          <DataProcessingNotice
            providerId={selectedProvider.id}
            providerName={selectedProvider.displayName}
            ollamaBaseUrl={ollamaBaseUrl}
            capabilities={selectedProvider.capabilities}
          />
        </>
      ) : null}

      {selectedProviderId ? (
        <ProviderCredentialsForm
          key={`${selectedProviderId}-${credKey}`}
          providerId={selectedProviderId}
          isLocal={isLocalProvider}
          hasStoredKey={hasStoredKey}
          onHasStoredKeyChange={setHasStoredKey}
          ollamaBaseUrl={ollamaBaseUrl}
          onOllamaBaseUrlChange={setOllamaBaseUrlState}
          ollamaAllowRemote={ollamaAllowRemote}
          onOllamaAllowRemoteChange={setOllamaAllowRemote}
          showDelete
          idPrefix="settings"
        />
      ) : null}

      {hasTranscription && transcriptionModels.length > 0 ? (
        <div className="ai-settings__field">
          <label htmlFor="transcription-model-select">Modèle audio (transcription)</label>
          <select
            id="transcription-model-select"
            value={transcriptionModel}
            disabled={busy || (diarizationEnabled && selectedProviderId === "openai")}
            onChange={(event) => {
              const next = event.target.value;
              setTranscriptionModel(next);
              void persistModelPrefs({ transcriptionModel: next });
            }}
          >
            {transcriptionModels.map((model) => (
              <option key={model.id} value={model.id}>
                {model.name}
              </option>
            ))}
          </select>
          {diarizationEnabled && selectedProviderId === "openai" ? (
            <p className="ai-settings__note" role="note">
              Avec la diarisation OpenAI, le modèle dédié <code>gpt-4o-transcribe-diarize</code> est
              utilisé automatiquement.
            </p>
          ) : null}
        </div>
      ) : null}

      {summaryModels.length > 0 ? (
        <div className="ai-settings__field">
          <label htmlFor="summary-model-select">Modèle LLM (compte-rendu)</label>
          <select
            id="summary-model-select"
            value={summaryModel}
            disabled={busy}
            onChange={(event) => {
              const next = event.target.value;
              setSummaryModel(next);
              void persistModelPrefs({ summaryModel: next });
            }}
          >
            {summaryModels.map((model) => (
              <option key={model.id} value={model.id}>
                {model.name}
              </option>
            ))}
          </select>
        </div>
      ) : null}

      {hasDiarization ? (
        <div className="ai-settings__field ai-settings__field--checkbox">
          <label htmlFor="diarization-toggle">
            <input
              id="diarization-toggle"
              type="checkbox"
              checked={diarizationEnabled}
              disabled={busy}
              onChange={(event) => {
                const next = event.target.checked;
                setDiarizationEnabled(next);
                void persistModelPrefs({ diarizationEnabled: next });
              }}
            />
            Identifier les locuteurs (diarisation)
          </label>
          <p className="ai-settings__note">
            Ajoute des labels de locuteurs dans la transcription. Utile pour les réunions à
            plusieurs voix.
          </p>
        </div>
      ) : null}

      {error ? (
        <p className="ai-settings__error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
