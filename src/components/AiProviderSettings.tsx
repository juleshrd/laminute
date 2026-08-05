import { useCallback, useEffect, useState } from "react";

import {
  deleteApiKey,
  getAiSettings,
  listAiProviders,
  saveApiKey,
  setOllamaBaseUrl,
  setSelectedProvider,
  validateApiKey,
} from "../lib/ai/api";
import type { KeyValidationResult, ProviderInfo } from "../lib/ai/types";
import { DataProcessingNotice } from "./DataProcessingNotice";
import "./AiProviderSettings.css";

const CAPABILITY_LABELS: Record<string, string> = {
  transcription: "Transcription",
  summary: "Résumé",
  local: "Local",
  streaming: "Streaming",
};

function formatCapabilities(provider: ProviderInfo): string {
  return (Object.entries(provider.capabilities) as [string, boolean][])
    .filter(([, enabled]) => enabled)
    .map(([key]) => CAPABILITY_LABELS[key] ?? key)
    .join(" · ");
}

export function AiProviderSettings() {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedProviderId, setSelectedProviderId] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [ollamaBaseUrl, setOllamaBaseUrlState] = useState("http://127.0.0.1:11434");
  const [hasStoredKey, setHasStoredKey] = useState(false);
  const [validation, setValidation] = useState<KeyValidationResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [providerList, settings] = await Promise.all([
        listAiProviders(),
        getAiSettings(),
      ]);
      setProviders(providerList);
      const initialId =
        settings.selectedProviderId ?? providerList[0]?.id ?? "";
      setSelectedProviderId(initialId);
      setHasStoredKey(settings.hasApiKey);
      setOllamaBaseUrlState(
        settings.ollamaBaseUrl ?? "http://127.0.0.1:11434",
      );
      setValidation(null);
      setApiKey("");
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
    setStatusMessage(null);
    setValidation(null);
    setApiKey("");
    try {
      const settings = await setSelectedProvider(nextId);
      setSelectedProviderId(nextId);
      setHasStoredKey(settings.hasApiKey);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sélection impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveOllamaBaseUrl() {
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      await setOllamaBaseUrl(ollamaBaseUrl);
      setStatusMessage("URL Ollama enregistrée.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Enregistrement impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function handleSave() {
    if (!selectedProviderId) return;
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      await saveApiKey(selectedProviderId, apiKey);
      setHasStoredKey(true);
      setApiKey("");
      setStatusMessage("Clé enregistrée dans le trousseau système.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Enregistrement impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function handleValidate() {
    if (!selectedProviderId) return;
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      const result = await validateApiKey(
        selectedProviderId,
        apiKey.trim() ? apiKey : undefined,
      );
      setValidation(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Validation impossible.");
      setValidation(null);
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteKey() {
    if (!selectedProviderId) return;
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    setValidation(null);
    try {
      await deleteApiKey(selectedProviderId);
      setHasStoredKey(false);
      setApiKey("");
      setStatusMessage("Clé supprimée du trousseau système.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Suppression impossible.");
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
          Sélectionnez un fournisseur et enregistrez votre clé API. Les secrets
          sont stockés dans le trousseau système, jamais en clair dans
          l&apos;application.
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

      {selectedProvider && (
        <>
          <p className="ai-settings__capabilities">
            Capacités : {formatCapabilities(selectedProvider)}
          </p>
          {!hasTranscription && (
            <p className="ai-settings__note" role="note">
              Ce fournisseur ne prend pas en charge la transcription audio. Vous
              pourrez générer un compte-rendu à partir d&apos;un texte collé.
            </p>
          )}
          <DataProcessingNotice
            providerName={selectedProvider.displayName}
            capabilities={selectedProvider.capabilities}
          />
        </>
      )}

      {selectedProviderId === "ollama" && (
        <div className="ai-settings__field">
          <label htmlFor="ollama-base-url">URL du serveur Ollama</label>
          <input
            id="ollama-base-url"
            type="url"
            value={ollamaBaseUrl}
            disabled={busy}
            onChange={(event) => setOllamaBaseUrlState(event.target.value)}
          />
          <button
            type="button"
            disabled={busy || !ollamaBaseUrl.trim()}
            onClick={() => void handleSaveOllamaBaseUrl()}
          >
            Enregistrer l&apos;URL
          </button>
        </div>
      )}

      {!isLocalProvider && (
        <div className="ai-settings__field">
          <label htmlFor="api-key-input">Clé API</label>
          <input
            id="api-key-input"
            type="password"
            autoComplete="off"
            placeholder={
              hasStoredKey
                ? "Clé enregistrée — saisir une nouvelle clé pour remplacer"
                : "Collez votre clé API"
            }
            value={apiKey}
            disabled={busy}
            onChange={(event) => setApiKey(event.target.value)}
          />
          {hasStoredKey && (
            <span className="ai-settings__badge" role="status">
              Clé enregistrée
            </span>
          )}
        </div>
      )}

      <div className="ai-settings__actions">
        {!isLocalProvider && (
          <button
            type="button"
            disabled={busy || !apiKey.trim()}
            onClick={() => void handleSave()}
          >
            Enregistrer
          </button>
        )}
        <button
          type="button"
          disabled={busy || (!isLocalProvider && !apiKey.trim() && !hasStoredKey)}
          onClick={() => void handleValidate()}
        >
          {isLocalProvider ? "Tester la connexion" : "Valider la clé"}
        </button>
        {!isLocalProvider && hasStoredKey && (
          <button
            type="button"
            className="ai-settings__danger"
            disabled={busy}
            onClick={() => void handleDeleteKey()}
          >
            Supprimer la clé
          </button>
        )}
      </div>

      {statusMessage && (
        <p className="ai-settings__status" role="status">
          {statusMessage}
        </p>
      )}

      {validation && (
        <div
          className={`ai-settings__validation ${
            validation.valid ? "ai-settings__validation--valid" : "ai-settings__validation--invalid"
          }`}
          role="status"
        >
          <strong>{validation.valid ? "Connexion valide" : "Connexion invalide"}</strong>
          <p>{validation.message}</p>
          {validation.models && validation.models.length > 0 && (
            <p>{validation.models.length} modèle(s) détecté(s).</p>
          )}
        </div>
      )}

      {error && (
        <p className="ai-settings__error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
