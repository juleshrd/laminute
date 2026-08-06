import type { StructuredSummary } from "./types";

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
      actions: parsed.actions ?? [],
      risques: parsed.risques ?? [],
      questionsOuvertes: parsed.questionsOuvertes ?? [],
    };
  } catch {
    return null;
  }
}
