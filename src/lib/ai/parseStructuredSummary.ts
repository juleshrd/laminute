import type {
  DecisionEntry,
  EvidenceSource,
  ItemOrigin,
  StructuredActionItem,
  StructuredDecisionItem,
  StructuredSummary,
} from "./types";

export function extractJsonPayload(raw: string): string {
  const trimmed = raw.trim();

  if (trimmed.includes("```")) {
    const start = trimmed.indexOf("```");
    const afterFence = trimmed.slice(start + 3);
    const content = afterFence.startsWith("json")
      ? afterFence.slice(4).trimStart()
      : afterFence.trimStart();
    const end = content.indexOf("```");
    if (end >= 0) {
      return content.slice(0, end).trim();
    }
  }

  return trimmed;
}

export function decisionText(decision: DecisionEntry): string {
  return typeof decision === "string" ? decision : decision.texte;
}

export function decisionAsItem(decision: DecisionEntry): StructuredDecisionItem {
  if (typeof decision === "string") {
    return { texte: decision, sources: [], origin: "generated" };
  }
  return {
    texte: decision.texte,
    id: decision.id,
    sources: decision.sources ?? [],
    origin: decision.origin ?? "generated",
  };
}

export function originLabel(origin: ItemOrigin | undefined): string {
  switch (origin) {
    case "validated":
      return "Validé";
    case "locked":
      return "Verrouillé";
    case "edited":
      return "Corrigé";
    default:
      return "Généré";
  }
}

export function formatEvidenceLabel(source: EvidenceSource): string {
  if (source.startMs != null && source.endMs != null) {
    const start = (source.startMs / 1000).toFixed(1);
    const end = (source.endMs / 1000).toFixed(1);
    return `${start}s–${end}s`;
  }
  if (source.quote) {
    return source.quote.slice(0, 40);
  }
  if (source.segmentIndex != null) {
    return `Segment ${source.segmentIndex + 1}`;
  }
  return "Preuve";
}

export function parseStoredSummary(content: string): StructuredSummary | null {
  try {
    const json = extractJsonPayload(content);
    const parsed = JSON.parse(json) as StructuredSummary;
    if (typeof parsed.synthese !== "string") {
      return null;
    }
    return {
      synthese: parsed.synthese,
      decisions: parsed.decisions ?? [],
      actions: (parsed.actions ?? []).map((action: StructuredActionItem) => ({
        ...action,
        sources: action.sources ?? [],
        origin: action.origin ?? "generated",
      })),
      risques: parsed.risques ?? [],
      questionsOuvertes: parsed.questionsOuvertes ?? [],
    };
  } catch {
    return null;
  }
}
