import { describe, expect, it } from "vitest";

import {
  buildDataProcessingNotice,
  onboardingWelcomeLead,
  resolvePrivacyDestination,
} from "./privacyNotices";
import { PRIVACY_POLICY_SHORT } from "./privacyPolicyShort";

describe("resolvePrivacyDestination", () => {
  it("classe Mistral et OpenAI en cloud", () => {
    expect(resolvePrivacyDestination("mistral")).toBe("mistral-cloud");
    expect(resolvePrivacyDestination("openai")).toBe("openai-cloud");
  });

  it("classe Ollama loopback vs distant", () => {
    expect(resolvePrivacyDestination("ollama", "http://127.0.0.1:11434")).toBe("ollama-loopback");
    expect(resolvePrivacyDestination("ollama", "http://localhost:11434")).toBe("ollama-loopback");
    expect(resolvePrivacyDestination("ollama", "http://192.168.1.10:11434")).toBe("ollama-remote");
  });
});

describe("buildDataProcessingNotice", () => {
  it("décrit Mistral avec auth Bearer TLS (pas « aucune clé »)", () => {
    const notice = buildDataProcessingNotice({
      providerId: "mistral",
      providerName: "Mistral AI",
      capabilities: { transcription: true, summary: true, local: false },
    });
    expect(notice.destination).toBe("mistral-cloud");
    expect(notice.auth).toBe("bearer-tls");
    expect(notice.body).toMatch(/TLS/);
    expect(notice.body).toMatch(/authentification/);
    expect(notice.body).not.toMatch(/Aucune clé API n['’]est incluse/);
    expect(notice.body).toMatch(/fichier audio/);
  });

  it("décrit OpenAI de la même façon", () => {
    const notice = buildDataProcessingNotice({
      providerId: "openai",
      providerName: "OpenAI",
      capabilities: { transcription: true, summary: true, local: false },
    });
    expect(notice.title).toContain("OpenAI");
    expect(notice.auth).toBe("bearer-tls");
    expect(notice.body).toMatch(/trousseau/);
  });

  it("qualifie Ollama de local uniquement en loopback", () => {
    const local = buildDataProcessingNotice({
      providerId: "ollama",
      providerName: "Ollama",
      ollamaBaseUrl: "http://127.0.0.1:11434",
      capabilities: { transcription: false, summary: true, local: true },
    });
    expect(local.destination).toBe("ollama-loopback");
    expect(local.body).toMatch(/loopback/i);
    expect(local.body).toMatch(/Aucune donnée/);

    const remote = buildDataProcessingNotice({
      providerId: "ollama",
      providerName: "Ollama",
      ollamaBaseUrl: "http://10.0.0.5:11434",
      capabilities: { transcription: false, summary: true, local: true },
    });
    expect(remote.destination).toBe("ollama-remote");
    expect(remote.body).toMatch(/10\.0\.0\.5:11434/);
    expect(remote.body).not.toMatch(/Aucune donnée n['’]est envoyée à un service cloud/);
  });
});

describe("textes d’accueil et politique courte", () => {
  it("ne promet plus que tout reste sur l’ordinateur", () => {
    const lead = onboardingWelcomeLead();
    expect(lead).not.toMatch(/Tout reste sur votre ordinateur/);
    expect(lead).toMatch(/stockées sur votre ordinateur/);
    expect(lead).toMatch(/fournisseur/);
  });

  it("mentionne Mistral, OpenAI et Ollama dans la politique courte", () => {
    expect(PRIVACY_POLICY_SHORT).toMatch(/Mistral/);
    expect(PRIVACY_POLICY_SHORT).toMatch(/OpenAI/);
    expect(PRIVACY_POLICY_SHORT).toMatch(/Ollama/);
    expect(PRIVACY_POLICY_SHORT).toMatch(/loopback/);
    expect(PRIVACY_POLICY_SHORT).toMatch(/TLS/);
  });
});
