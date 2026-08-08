import { useState } from "react";

import { deleteApiKey, saveApiKey, setOllamaBaseUrl, validateApiKey } from "../lib/ai/api";
import { isOllamaLoopbackUrl } from "../lib/ai/ollamaUrl";
import type { KeyValidationResult } from "../lib/ai/types";

export interface ProviderCredentialsFormProps {
  providerId: string;
  isLocal: boolean;
  hasStoredKey: boolean;
  onHasStoredKeyChange?: (hasKey: boolean) => void;
  ollamaBaseUrl: string;
  onOllamaBaseUrlChange: (url: string) => void;
  ollamaAllowRemote?: boolean;
  onOllamaAllowRemoteChange?: (allow: boolean) => void;
  showDelete?: boolean;
  idPrefix?: string;
}

export function ProviderCredentialsForm({
  providerId,
  isLocal,
  hasStoredKey,
  onHasStoredKeyChange,
  ollamaBaseUrl,
  onOllamaBaseUrlChange,
  ollamaAllowRemote = false,
  onOllamaAllowRemoteChange,
  showDelete = true,
  idPrefix = "cred",
}: ProviderCredentialsFormProps) {
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [validation, setValidation] = useState<KeyValidationResult | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const ollamaInputId = `${idPrefix}-ollama-url`;
  const ollamaRemoteId = `${idPrefix}-ollama-remote`;
  const apiKeyInputId = `${idPrefix}-api-key`;
  const isLoopback = isOllamaLoopbackUrl(ollamaBaseUrl);
  const looksRemote = Boolean(ollamaBaseUrl.trim()) && !isLoopback;
  const canSaveOllama = Boolean(ollamaBaseUrl.trim()) && (!looksRemote || ollamaAllowRemote);

  async function handleSaveOllamaBaseUrl() {
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      await setOllamaBaseUrl(ollamaBaseUrl, looksRemote ? ollamaAllowRemote : false);
      setStatusMessage(
        looksRemote
          ? "URL Ollama distante enregistrée (opt-in activé)."
          : "URL Ollama locale enregistrée.",
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "Enregistrement impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function handleSave() {
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      await saveApiKey(providerId, apiKey);
      onHasStoredKeyChange?.(true);
      setApiKey("");
      setStatusMessage("Clé enregistrée dans le trousseau système.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Enregistrement impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function handleValidate() {
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      const result = await validateApiKey(providerId, apiKey.trim() ? apiKey : undefined);
      setValidation(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Validation impossible.");
      setValidation(null);
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteKey() {
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    setValidation(null);
    try {
      await deleteApiKey(providerId);
      onHasStoredKeyChange?.(false);
      setApiKey("");
      setStatusMessage("Clé supprimée du trousseau système.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Suppression impossible.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="ai-settings__credentials">
      {isLocal ? (
        <div className="ai-settings__field">
          <label htmlFor={ollamaInputId}>URL du serveur Ollama</label>
          <input
            id={ollamaInputId}
            type="url"
            value={ollamaBaseUrl}
            disabled={busy}
            onChange={(event) => onOllamaBaseUrlChange(event.target.value)}
            placeholder="http://127.0.0.1:11434"
          />
          {isLoopback ? (
            <p className="ai-settings__note" role="status">
              Serveur local (loopback) — les transcriptions restent sur cette machine.
            </p>
          ) : null}
          {looksRemote ? (
            <>
              <p className="ai-settings__note ai-settings__note--warn" role="status">
                Serveur distant ou LAN détecté — les textes de réunion seront envoyés à cette
                origine.
              </p>
              <div className="ai-settings__field ai-settings__field--checkbox">
                <label htmlFor={ollamaRemoteId}>
                  <input
                    id={ollamaRemoteId}
                    type="checkbox"
                    checked={ollamaAllowRemote}
                    disabled={busy}
                    onChange={(event) => onOllamaAllowRemoteChange?.(event.target.checked)}
                  />
                  J&apos;autorise explicitement ce serveur Ollama hors de cette machine
                </label>
              </div>
            </>
          ) : null}
          <button
            type="button"
            disabled={busy || !canSaveOllama}
            onClick={() => void handleSaveOllamaBaseUrl()}
          >
            Enregistrer l&apos;URL
          </button>
        </div>
      ) : (
        <div className="ai-settings__field">
          <label htmlFor={apiKeyInputId}>Clé API</label>
          <input
            id={apiKeyInputId}
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
          {hasStoredKey ? (
            <span className="ai-settings__badge" role="status">
              Clé enregistrée
            </span>
          ) : null}
        </div>
      )}

      <div className="ai-settings__actions">
        {!isLocal ? (
          <button type="button" disabled={busy || !apiKey.trim()} onClick={() => void handleSave()}>
            Enregistrer
          </button>
        ) : null}
        <button
          type="button"
          disabled={busy || (!isLocal && !apiKey.trim() && !hasStoredKey)}
          onClick={() => void handleValidate()}
        >
          {isLocal ? "Tester la connexion" : "Valider la clé"}
        </button>
        {!isLocal && showDelete && hasStoredKey ? (
          <button
            type="button"
            className="ai-settings__danger"
            disabled={busy}
            onClick={() => void handleDeleteKey()}
          >
            Supprimer la clé
          </button>
        ) : null}
      </div>

      {statusMessage ? (
        <p className="ai-settings__status" role="status">
          {statusMessage}
        </p>
      ) : null}

      {validation ? (
        <div
          className={`ai-settings__validation ${
            validation.valid ? "ai-settings__validation--valid" : "ai-settings__validation--invalid"
          }`}
          role="status"
        >
          <strong>{validation.valid ? "Connexion valide" : "Connexion invalide"}</strong>
          <p>{validation.message}</p>
          {validation.models && validation.models.length > 0 ? (
            <p>{validation.models.length} modèle(s) détecté(s).</p>
          ) : null}
        </div>
      ) : null}

      {error ? (
        <p className="ai-settings__error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
