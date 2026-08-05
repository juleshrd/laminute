import { useState } from "react";

import { generateStructuredSummary } from "../lib/ai/api";
import type { GenerateStructuredSummaryOutput } from "../lib/ai/types";
import "./StructuredSummaryPanel.css";

async function copyText(value: string) {
  await navigator.clipboard.writeText(value);
}

function formatActions(output: GenerateStructuredSummaryOutput): string {
  const lines = output.structured.actions.map((action) => {
    const parts = [action.titre];
    if (action.responsable) parts.push(`(${action.responsable})`);
    if (action.echeance) parts.push(`— ${action.echeance}`);
    return `- ${parts.join(" ")}`;
  });
  return lines.join("\n");
}

export function StructuredSummaryPanel() {
  const [meetingId, setMeetingId] = useState("");
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<GenerateStructuredSummaryOutput | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  async function handleGenerate() {
    const trimmedText = text.trim();
    const trimmedMeetingId = meetingId.trim();

    if (!trimmedText && !trimmedMeetingId) {
      setError("Saisissez un identifiant de réunion ou un texte de transcription.");
      return;
    }

    setBusy(true);
    setError(null);
    setCopied(null);

    try {
      const output = await generateStructuredSummary({
        meetingId: trimmedMeetingId || undefined,
        text: trimmedText || undefined,
      });
      setResult(output);
    } catch (err) {
      setResult(null);
      setError(err instanceof Error ? err.message : "Génération impossible.");
    } finally {
      setBusy(false);
    }
  }

  async function handleCopy(label: string, value: string) {
    await copyText(value);
    setCopied(label);
  }

  return (
    <section className="structured-summary" aria-labelledby="structured-summary-title">
      <header className="structured-summary__header">
        <h2 id="structured-summary-title">Compte-rendu structuré</h2>
        <p>
          Collez une transcription ou indiquez l&apos;identifiant d&apos;une réunion existante, puis
          générez un compte-rendu avec synthèse, décisions et actions.
        </p>
      </header>

      <div className="structured-summary__field">
        <label htmlFor="summary-meeting-id">Identifiant de réunion (optionnel)</label>
        <input
          id="summary-meeting-id"
          type="text"
          value={meetingId}
          disabled={busy}
          placeholder="UUID de la réunion"
          onChange={(event) => setMeetingId(event.target.value)}
        />
      </div>

      <div className="structured-summary__field">
        <label htmlFor="summary-text">Texte de transcription (optionnel)</label>
        <textarea
          id="summary-text"
          rows={8}
          value={text}
          disabled={busy}
          placeholder="Collez ici la transcription de la réunion…"
          onChange={(event) => setText(event.target.value)}
        />
      </div>

      <div className="structured-summary__actions">
        <button type="button" disabled={busy} onClick={() => void handleGenerate()}>
          {busy ? "Génération…" : "Générer"}
        </button>
      </div>

      {error && (
        <p className="structured-summary__error" role="alert">
          {error}
        </p>
      )}

      {result && (
        <div className="structured-summary__result">
          <p className="structured-summary__meta">
            Réunion <span className="mono">{result.meetingId}</span>
          </p>

          <article className="structured-summary__block">
            <div className="structured-summary__block-header">
              <h3>Synthèse</h3>
              <button
                type="button"
                onClick={() => void handleCopy("synthese", result.structured.synthese)}
              >
                Copier
              </button>
            </div>
            <p>{result.structured.synthese}</p>
          </article>

          <article className="structured-summary__block">
            <div className="structured-summary__block-header">
              <h3>Décisions</h3>
              <button
                type="button"
                onClick={() => void handleCopy("decisions", result.structured.decisions.join("\n"))}
              >
                Copier
              </button>
            </div>
            {result.structured.decisions.length > 0 ? (
              <ul>
                {result.structured.decisions.map((decision) => (
                  <li key={decision}>{decision}</li>
                ))}
              </ul>
            ) : (
              <p className="structured-summary__empty">Aucune décision identifiée.</p>
            )}
          </article>

          <article className="structured-summary__block">
            <div className="structured-summary__block-header">
              <h3>Actions</h3>
              <button
                type="button"
                onClick={() => void handleCopy("actions", formatActions(result))}
              >
                Copier
              </button>
            </div>
            {result.structured.actions.length > 0 ? (
              <ul>
                {result.structured.actions.map((action) => (
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

          {copied && (
            <p className="structured-summary__status" role="status">
              {copied === "synthese" && "Synthèse copiée."}
              {copied === "decisions" && "Décisions copiées."}
              {copied === "actions" && "Actions copiées."}
            </p>
          )}
        </div>
      )}
    </section>
  );
}
