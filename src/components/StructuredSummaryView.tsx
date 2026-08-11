import { useState, type ReactNode } from "react";

import {
  decisionAsItem,
  decisionText,
  formatEvidenceLabel,
  originLabel,
} from "../lib/ai/parseStructuredSummary";
import type {
  EvidenceSource,
  ItemOrigin,
  StructuredActionItem,
  StructuredSummary,
  SummaryValidationState,
} from "../lib/ai/types";
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

export interface SummaryEvidenceRequest {
  sources: EvidenceSource[];
  label: string;
}

interface StructuredSummaryViewProps {
  summary: StructuredSummary;
  providerId?: string;
  model?: string;
  validationState?: SummaryValidationState;
  generatedAt?: string;
  showCopy?: boolean;
  editable?: boolean;
  headingLevel?: 2 | 3 | 4;
  onChange?: (summary: StructuredSummary) => void;
  onValidateSummary?: () => void;
  onEvidenceRequest?: (request: SummaryEvidenceRequest) => void;
}

function SectionHeading({ level, children }: { level: 2 | 3 | 4; children: ReactNode }) {
  if (level === 2) {
    return <h2>{children}</h2>;
  }
  if (level === 4) {
    return <h4>{children}</h4>;
  }
  return <h3>{children}</h3>;
}

function OriginChip({ origin }: { origin?: ItemOrigin }) {
  return <span className={`summary-origin summary-origin--${origin ?? "generated"}`}>{originLabel(origin)}</span>;
}

function EvidenceButtons({
  sources,
  label,
  onEvidenceRequest,
}: {
  sources?: EvidenceSource[];
  label: string;
  onEvidenceRequest?: (request: SummaryEvidenceRequest) => void;
}) {
  if (!sources?.length || !onEvidenceRequest) {
    return null;
  }
  return (
    <div className="summary-evidence">
      {sources.map((source, index) => (
        <button
          key={`${label}-${index}`}
          type="button"
          className="summary-evidence__button"
          onClick={() => onEvidenceRequest({ sources: [source], label })}
        >
          Voir la preuve · {formatEvidenceLabel(source)}
        </button>
      ))}
    </div>
  );
}

export function StructuredSummaryView({
  summary,
  providerId,
  model,
  validationState,
  generatedAt,
  showCopy = false,
  editable = false,
  headingLevel = 3,
  onChange,
  onValidateSummary,
  onEvidenceRequest,
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

  function updateDecision(index: number, patch: Partial<{ texte: string; origin: ItemOrigin }>) {
    if (!onChange) return;
    const decisions = summary.decisions.map((decision, i) => {
      if (i !== index) return decision;
      const item = decisionAsItem(decision);
      return {
        ...item,
        ...patch,
        origin: patch.origin ?? (patch.texte != null ? "edited" : item.origin),
      };
    });
    onChange({ ...summary, decisions });
  }

  function updateAction(index: number, patch: Partial<StructuredActionItem>) {
    if (!onChange) return;
    const actions = summary.actions.map((action, i) => {
      if (i !== index) return action;
      return {
        ...action,
        ...patch,
        origin: patch.origin ?? (patch.titre != null || patch.responsable != null || patch.echeance != null || patch.description != null ? "edited" : action.origin),
      };
    });
    onChange({ ...summary, actions });
  }

  function removeDecision(index: number) {
    if (!onChange) return;
    onChange({
      ...summary,
      decisions: summary.decisions.filter((_, i) => i !== index),
    });
  }

  function removeAction(index: number) {
    if (!onChange) return;
    onChange({
      ...summary,
      actions: summary.actions.filter((_, i) => i !== index),
    });
  }

  function addDecision() {
    if (!onChange) return;
    onChange({
      ...summary,
      decisions: [
        ...summary.decisions,
        { texte: "Nouvelle décision", origin: "edited", sources: [] },
      ],
    });
  }

  function addAction() {
    if (!onChange) return;
    onChange({
      ...summary,
      actions: [
        ...summary.actions,
        { titre: "Nouvelle action", origin: "edited", sources: [] },
      ],
    });
  }

  const validationLabel =
    validationState === "validated"
      ? "Validé"
      : validationState === "edited"
        ? "Corrigé"
        : "Généré";

  return (
    <div className="structured-summary__result">
      <p className="structured-summary__meta">
        {[providerId ? `Fournisseur : ${providerId}` : null, model ? `Modèle : ${model}` : null, generatedAt ? `Généré : ${generatedAt}` : null, `Validation : ${validationLabel}`]
          .filter(Boolean)
          .join(" · ")}
      </p>

      <article className="structured-summary__block">
        {sectionHeader("Synthèse", "synthese", summary.synthese)}
        {editable ? (
          <textarea
            className="summary-edit-field"
            value={summary.synthese}
            onChange={(event) => onChange?.({ ...summary, synthese: event.target.value })}
            rows={4}
          />
        ) : (
          <p>{summary.synthese}</p>
        )}
      </article>

      <article className="structured-summary__block">
        {sectionHeader(
          "Décisions",
          "decisions",
          summary.decisions.map((decision) => decisionText(decision)).join("\n"),
        )}
        {summary.decisions.length > 0 ? (
          <ul className="summary-editable-list">
            {summary.decisions.map((decision, index) => {
              const item = decisionAsItem(decision);
              return (
                <li key={item.id ?? `${item.texte}-${index}`}>
                  <div className="summary-item-row">
                    {editable ? (
                      <input
                        className="summary-edit-field"
                        value={item.texte}
                        onChange={(event) => updateDecision(index, { texte: event.target.value })}
                      />
                    ) : (
                      <span>{item.texte}</span>
                    )}
                    <OriginChip origin={item.origin} />
                  </div>
                  <EvidenceButtons
                    sources={item.sources}
                    label={item.texte}
                    onEvidenceRequest={onEvidenceRequest}
                  />
                  {editable && (
                    <div className="summary-item-actions">
                      <button type="button" onClick={() => updateDecision(index, { origin: "validated" })}>
                        Valider
                      </button>
                      <button type="button" onClick={() => updateDecision(index, { origin: "locked" })}>
                        Verrouiller
                      </button>
                      <button type="button" onClick={() => removeDecision(index)}>
                        Supprimer
                      </button>
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        ) : (
          <p className="structured-summary__empty">Aucune décision identifiée.</p>
        )}
        {editable && (
          <button type="button" className="summary-add-button" onClick={addDecision}>
            Ajouter une décision
          </button>
        )}
      </article>

      <article className="structured-summary__block">
        {sectionHeader("Actions", "actions", formatActions(summary))}
        {summary.actions.length > 0 ? (
          <ul className="summary-editable-list">
            {summary.actions.map((action, index) => (
              <li key={action.id ?? `${action.titre}-${index}`}>
                <div className="summary-item-row">
                  {editable ? (
                    <div className="summary-action-fields">
                      <input
                        className="summary-edit-field"
                        value={action.titre}
                        onChange={(event) => updateAction(index, { titre: event.target.value })}
                        aria-label="Titre de l'action"
                      />
                      <input
                        className="summary-edit-field"
                        value={action.responsable ?? ""}
                        placeholder="Responsable"
                        onChange={(event) =>
                          updateAction(index, {
                            responsable: event.target.value || undefined,
                          })
                        }
                      />
                      <input
                        className="summary-edit-field"
                        value={action.echeance ?? ""}
                        placeholder="Échéance"
                        onChange={(event) =>
                          updateAction(index, {
                            echeance: event.target.value || undefined,
                          })
                        }
                      />
                    </div>
                  ) : (
                    <span>
                      <strong>{action.titre}</strong>
                      {action.description && <span> — {action.description}</span>}
                      {action.responsable && (
                        <span className="structured-summary__tag">{action.responsable}</span>
                      )}
                      {action.echeance && (
                        <span className="structured-summary__tag">{action.echeance}</span>
                      )}
                    </span>
                  )}
                  <OriginChip origin={action.origin} />
                </div>
                <EvidenceButtons
                  sources={action.sources}
                  label={action.titre}
                  onEvidenceRequest={onEvidenceRequest}
                />
                {editable && (
                  <div className="summary-item-actions">
                    <button type="button" onClick={() => updateAction(index, { origin: "validated" })}>
                      Valider
                    </button>
                    <button type="button" onClick={() => updateAction(index, { origin: "locked" })}>
                      Verrouiller
                    </button>
                    <button type="button" onClick={() => removeAction(index)}>
                      Supprimer
                    </button>
                  </div>
                )}
              </li>
            ))}
          </ul>
        ) : (
          <p className="structured-summary__empty">Aucune action identifiée.</p>
        )}
        {editable && (
          <button type="button" className="summary-add-button" onClick={addAction}>
            Ajouter une action
          </button>
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

      {editable && onValidateSummary && (
        <div className="structured-summary__actions">
          <button type="button" onClick={onValidateSummary}>
            Valider le compte-rendu
          </button>
        </div>
      )}

      {copied && (
        <p className="structured-summary__status" role="status">
          {COPY_LABELS[copied]}
        </p>
      )}
    </div>
  );
}
