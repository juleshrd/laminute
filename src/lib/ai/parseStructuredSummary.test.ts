import { describe, expect, it } from "vitest";

import { extractJsonPayload, parseStoredSummary } from "./parseStructuredSummary";

describe("parseStoredSummary", () => {
  it("parse un JSON brut valide", () => {
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
    const content =
      '```json\n{"synthese":"OK","decisions":[],"actions":[],"risques":[],"questionsOuvertes":[]}\n```';
    const parsed = parseStoredSummary(content);
    expect(parsed?.synthese).toBe("OK");
  });

  it("applique des tableaux vides pour les champs absents", () => {
    const content = JSON.stringify({ synthese: "Synthèse seule" });
    const parsed = parseStoredSummary(content);
    expect(parsed).toEqual({
      synthese: "Synthèse seule",
      decisions: [],
      actions: [],
      risques: [],
      questionsOuvertes: [],
    });
  });

  it("retourne null pour un JSON invalide", () => {
    expect(parseStoredSummary("pas du json")).toBeNull();
  });

  it("conserve risques et questionsOuvertes lorsqu'ils sont présents", () => {
    const content = JSON.stringify({
      synthese: "Bilan",
      decisions: [],
      actions: [],
      risques: ["Retard fournisseur"],
      questionsOuvertes: ["Budget Q3 ?"],
    });
    const parsed = parseStoredSummary(content);
    expect(parsed?.risques).toEqual(["Retard fournisseur"]);
    expect(parsed?.questionsOuvertes).toEqual(["Budget Q3 ?"]);
  });
});

describe("extractJsonPayload", () => {
  it("extrait le JSON d'une fence markdown", () => {
    const raw = '```json\n{"synthese":"x"}\n```';
    expect(extractJsonPayload(raw)).toBe('{"synthese":"x"}');
  });
});
