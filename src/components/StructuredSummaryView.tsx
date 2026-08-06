import { useState, type ReactNode } from "react";

import type { StructuredSummary } from "../lib/ai/types";
import "./StructuredSummaryPanel.css";

async function copyText(value: string) {
  await navigator.clipboard.writeText(value);
}

function formatActions(summary: StructuredSummary): string {
  const lines = summary.actions.map((action) => {
    const parts = [action.titre];
    if (action.responsable) parts.push(`(${action.responsable})`);
    if (action.echeance) parts.push(`— ${action.echeance}`);
    return `- ${parts.join(" ")}`;
  });
  return lines.join("\n");
}

type SectionKey = "synthese" | "decisions" | "actions" | "risques" | "questionsOuvertes";

const COPY_LABELS: Record<SectionKey, string> = {
  synthese: "Synthèse copiée.",
  decisions: "Décisions copiées.",
  actions: "Actions copiées.",
  risques: "Risques copiés.",
  questionsOuvertes: "Questions ouvertes copiées.",
};

interface StructuredSummaryViewProps {
  summary: StructuredSummary;
  providerId?: string;
  showCopy?: boolean;
  headingLevel?: 2 | 3 | 4;
}

function SectionHeading({
  level,
  children,
}: {
  level: 2 | 3 | 4;
  children: ReactNode;
}) {
  if (level === 2) {
    return <h2>{children}</h2>;
  }
  if (level === 4) {
    return <h4>{children}</h4>;
  }
  return <h3>{children}</h3>;
}

export function StructuredSummaryView({
  summary,
  providerId,
  showCopy = false,
  headingLevel = 3,
}: StructuredSummaryViewProps) {
  const [copied, setCopied] = useState<SectionKey | null>(null);

  async function handleCopy(key: SectionKey, value: string) {
    await copyText(value);
    setCopied(key);
  }

  function sectionHeader(title: string, key: SectionKey, copyValue: string) {
    if (!showCopy) {
      return <SectionHeading level={headingLevel}>{title}</SectionHeading>;
    }

    return (
      <div className="structured-summary__block-header">
        <SectionHeading level={headingLevel}>{title}</SectionHeading>
        <button type="button" onClick={() => void handleCopy(key, copyValue)}>
          Copier
        </button>
      </div>
    );
  }

  return (
    <div className="structured-summary__result">
      {providerId && <p className="structured-summary__meta">Fournisseur : {providerId}</p>}

      <article className="structured-summary__block">
        {sectionHeader("Synthèse", "synthese", summary.synthese)}
        <p>{summary.synthese}</p>
      </article>

      <article className="structured-summary__block">
        {sectionHeader("Décisions", "decisions", summary.decisions.join("\n"))}
        {summary.decisions.length > 0 ? (
          <ul>
            {summary.decisions.map((decision) => (
              <li key={decision}>{decision}</li>
            ))}
          </ul>
        ) : (
          <p className="structured-summary__empty">Aucune décision identifiée.</p>
        )}
      </article>

      <article className="structured-summary__block">
        {sectionHeader("Actions", "actions", formatActions(summary))}
        {summary.actions.length > 0 ? (
          <ul>
            {summary.actions.map((action) => (
              <li key={`${action.titre}-${action.responsable ?? ""}`}>
                <strong>{action.titre}</strong>
                {action.description && <span> — {action.description}</span>}
                {action.responsable && (
                  <span className="structured-summary__tag">{action.responsable}</span>
                )}
                {action.echeance && (
                  <span className="structured-summary__tag">{action.echeance}</span>
                )}
              </li>
            ))}
          </ul>
        ) : (
          <p className="structured-summary__empty">Aucune action identifiée.</p>
        )}
      </article>

      <article className="structured-summary__block">
        {sectionHeader("Risques", "risques", summary.risques.join("\n"))}
        {summary.risques.length > 0 ? (
          <ul>
            {summary.risques.map((risque) => (
              <li key={risque}>{risque}</li>
            ))}
          </ul>
        ) : (
          <p className="structured-summary__empty">Aucun risque identifié.</p>
        )}
      </article>

      <article className="structured-summary__block">
        {sectionHeader(
          "Questions ouvertes",
          "questionsOuvertes",
          summary.questionsOuvertes.join("\n"),
        )}
        {summary.questionsOuvertes.length > 0 ? (
          <ul>
            {summary.questionsOuvertes.map((question) => (
              <li key={question}>{question}</li>
            ))}
          </ul>
        ) : (
          <p className="structured-summary__empty">Aucune question ouverte identifiée.</p>
        )}
      </article>

      {copied && (
        <p className="structured-summary__status" role="status">
          {COPY_LABELS[copied]}
        </p>
      )}
    </div>
  );
}
