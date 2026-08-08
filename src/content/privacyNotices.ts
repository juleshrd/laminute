import { isOllamaLoopbackUrl } from "../lib/ai/ollamaUrl";

export type PrivacyProviderId = "mistral" | "openai" | "ollama" | string;

export type PrivacyDestination =
  "local-storage" | "mistral-cloud" | "openai-cloud" | "ollama-loopback" | "ollama-remote";

export type PrivacyAuth = "none" | "bearer-tls";

export interface PrivacyNoticeContext {
  providerId: PrivacyProviderId;
  providerName: string;
  /** URL Ollama configurée ; ignorée pour les autres fournisseurs. */
  ollamaBaseUrl?: string | null;
  /** Capacités déclarées par le registre (transcription notamment). */
  capabilities?: {
    transcription: boolean;
    summary: boolean;
    local: boolean;
  };
}

export interface PrivacyNoticeCopy {
  title: string;
  body: string;
  destination: PrivacyDestination;
  auth: PrivacyAuth;
}

const CLOUD_AUTH_FOOTER =
  "Votre clé API est transmise au fournisseur via TLS pour authentification ; elle n’est jamais intégrée au contenu métier ni aux exports. Au repos, elle reste dans le trousseau système.";

export function resolvePrivacyDestination(
  providerId: PrivacyProviderId,
  ollamaBaseUrl?: string | null,
): PrivacyDestination {
  if (providerId === "mistral") return "mistral-cloud";
  if (providerId === "openai") return "openai-cloud";
  if (providerId === "ollama") {
    return isOllamaLoopbackUrl(ollamaBaseUrl ?? "") ? "ollama-loopback" : "ollama-remote";
  }
  // Fallback : si le registre marque local, traiter comme Ollama loopback uniquement si URL loopback.
  return "mistral-cloud";
}

export function buildDataProcessingNotice(ctx: PrivacyNoticeContext): PrivacyNoticeCopy {
  const destination = resolvePrivacyDestination(ctx.providerId, ctx.ollamaBaseUrl);
  const hasTranscription = ctx.capabilities?.transcription ?? destination !== "ollama-loopback";

  if (destination === "ollama-loopback") {
    return {
      title: `Traitement local via ${ctx.providerName}`,
      body: "le compte-rendu est généré sur votre machine (URL loopback). Aucune donnée n’est envoyée à un service cloud. La transcription audio n’est pas disponible avec ce fournisseur.",
      destination,
      auth: "none",
    };
  }

  if (destination === "ollama-remote") {
    const origin = safeOrigin(ctx.ollamaBaseUrl) ?? "le serveur Ollama configuré";
    return {
      title: `Traitement distant via ${ctx.providerName}`,
      body: `le texte du compte-rendu est envoyé à ${origin} (serveur non loopback, opt-in distant). Aucune clé API n’est transmise. La transcription audio n’est pas disponible avec ce fournisseur.`,
      destination,
      auth: "none",
    };
  }

  const payload = hasTranscription
    ? "la transcription transmet le fichier audio ; le compte-rendu transmet uniquement le texte transcrit."
    : "le compte-rendu transmet uniquement le texte fourni.";

  return {
    title: `Données envoyées à ${ctx.providerName}`,
    body: `${payload} ${CLOUD_AUTH_FOOTER}`,
    destination,
    auth: "bearer-tls",
  };
}

export function onboardingWelcomeLead(): string {
  return "Enregistrez ou importez un audio, lancez le traitement, et consultez le résultat. Les réunions restent stockées sur votre ordinateur ; le traitement dépend du fournisseur IA que vous choisissez (cloud ou Ollama).";
}

export function onboardingProviderDescription(
  providerId: PrivacyProviderId,
  capabilities: { local: boolean; transcription: boolean },
): string {
  if (providerId === "ollama" || capabilities.local) {
    return "Compte-rendu via Ollama (local si URL loopback).";
  }
  if (capabilities.transcription) {
    return "Transcription + compte-rendu (envoi cloud).";
  }
  return "Compte-rendu à partir d’un texte (envoi cloud).";
}

export function privacySettingsIntro(): string {
  return "Vos réunions restent sur cet ordinateur jusqu’à suppression. Les clés API (Mistral, OpenAI) sont stockées dans le trousseau système et transmises au fournisseur via TLS pour authentification uniquement (réglages IA).";
}

function safeOrigin(raw?: string | null): string | null {
  if (!raw?.trim()) return null;
  try {
    return new URL(raw.trim()).origin;
  } catch {
    return null;
  }
}
