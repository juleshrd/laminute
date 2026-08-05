import { describe, expect, it } from "vitest";

import {
  formatDurationMs,
  meetingStatusLabel,
  parseStoredSummary,
} from "./meetings";

describe("meetings", () => {
  it("affiche les libellés de statut en français", () => {
    expect(meetingStatusLabel("draft")).toBe("Brouillon");
    expect(meetingStatusLabel("completed")).toBe("Terminée");
  });

  it("parse un compte-rendu structuré stocké en JSON", () => {
    const content = JSON.stringify({
      synthese: "Réunion productive",
      decisions: ["Valider le planning"],
      actions: [{ titre: "Envoyer le CR" }],
      risques: [],
      questionsOuvertes: [],
    });

    const parsed = parseStoredSummary(content);
    expect(parsed?.synthese).toBe("Réunion productive");
    expect(parsed?.decisions).toEqual(["Valider le planning"]);
    expect(parsed?.actions[0]?.titre).toBe("Envoyer le CR");
  });

  it("parse un JSON entouré de balises markdown", () => {
    const content = '```json\n{"synthese":"OK","decisions":[],"actions":[],"risques":[],"questionsOuvertes":[]}\n```';
    const parsed = parseStoredSummary(content);
    expect(parsed?.synthese).toBe("OK");
  });

  it("retourne null pour un contenu invalide", () => {
    expect(parseStoredSummary("pas du json")).toBeNull();
  });

  it("formate une durée en minutes:secondes", () => {
    expect(formatDurationMs(125_000)).toBe("2:05");
    expect(formatDurationMs(null)).toBe("—");
  });
});
