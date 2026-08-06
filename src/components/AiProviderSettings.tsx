import { useCallback, useEffect, useState } from "react";

import {
  getAiSettings,
  listAiProviders,
  setSelectedProvider,
} from "../lib/ai/api";
import type { ProviderInfo } from "../lib/ai/types";
import { formatCapabilities } from "../lib/ai/formatCapabilities";
import { DataProcessingNotice } from "./DataProcessingNotice";
import { ProviderCredentialsForm } from "./ProviderCredentialsForm";
import "./AiProviderSettings.css";

export function AiProviderSettings() {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedProviderId, setSelectedProviderId] = useState("");
  const [ollamaBaseUrl, setOllamaBaseUrlState] = useState("http://127.0.0.1:11434");
  const [hasStoredKey, setHasStoredKey] = useState(false);
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
      setSelectedProviderId(initialId);
      setHasStoredKey(settings.hasApiKey);
      setOllamaBaseUrlState(settings.ollamaBaseUrl ?? "http://127.0.0.1:11434");
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

  async function handleProviderChange(nextId: string) {
    setBusy(true);
    setError(null);
    try {
      const settings = await setSelectedProvider(nextId);
      setSelectedProviderId(nextId);
      setHasStoredKey(settings.hasApiKey);
      setCredKey((k) => k + 1);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sélection impossible.");
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
          Sélectionnez un fournisseur et enregistrez votre clé API. Les secrets sont stockés dans le
          trousseau système, jamais en clair dans l&apos;application.
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
            providerName={selectedProvider.displayName}
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
          showDelete
          idPrefix="settings"
        />
      ) : null}

      {error ? (
        <p className="ai-settings__error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
