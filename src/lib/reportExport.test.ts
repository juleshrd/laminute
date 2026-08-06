import { describe, expect, it } from "vitest";

import type { StructuredSummary } from "./ai/types";
import { buildReportMarkdown, type ReportExportMeta } from "./reportExport";

const meta: ReportExportMeta = {
  title: "Comité produit",
  statusLabel: "Terminée",
  displayDate: "5 août 2026 à 14:00",
  durationLabel: "12:30",
};

const summary: StructuredSummary = {
  synthese: "Point d’avancement du trimestre.",
  decisions: ["Valider le roadmap Q3"],
  actions: [
    {
      titre: "Rédiger le brief",
      description: "Version courte",
      responsable: "Alice",
      echeance: "2026-08-12",
    },
  ],
  risques: ["Délai serré"],
  questionsOuvertes: ["Budget marketing ?"],
};

describe("buildReportMarkdown", () => {
  it("inclut le titre, les métadonnées et les sections du compte-rendu", () => {
    const md = buildReportMarkdown(meta, summary);

    expect(md).toContain("# Comité produit");
    expect(md).toContain("La Minute");
    expect(md).toContain("| Statut | Terminée |");
    expect(md).toContain("| Date | 5 août 2026 à 14:00 |");
    expect(md).toContain("| Durée | 12:30 |");
    expect(md).toContain("## Synthèse");
    expect(md).toContain("Point d’avancement du trimestre.");
    expect(md).toContain("## Décisions");
    expect(md).toContain("- Valider le roadmap Q3");
    expect(md).toContain("## Actions");
    expect(md).toContain("**Rédiger le brief**");
    expect(md).toContain("responsable : Alice");
    expect(md).toContain("échéance : 2026-08-12");
    expect(md).toContain("## Risques");
    expect(md).toContain("- Délai serré");
    expect(md).toContain("## Questions ouvertes");
    expect(md).toContain("- Budget marketing ?");
  });

  it("omet risques et questions si vides, et gère les listes vides", () => {
    const empty: StructuredSummary = {
      synthese: "OK",
      decisions: [],
      actions: [],
      risques: [],
      questionsOuvertes: [],
    };
    const md = buildReportMarkdown(meta, empty);

    expect(md).toContain("_Aucun élément._");
    expect(md).toContain("_Aucune action identifiée._");
    expect(md).not.toContain("## Risques");
    expect(md).not.toContain("## Questions ouvertes");
  });
});
