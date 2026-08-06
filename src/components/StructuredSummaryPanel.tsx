import { useState } from "react";

import { generateStructuredSummary } from "../lib/ai/api";
import type { GenerateStructuredSummaryOutput } from "../lib/ai/types";
import { StructuredSummaryView } from "./StructuredSummaryView";
import "./StructuredSummaryPanel.css";

export function StructuredSummaryPanel() {
  const [meetingId, setMeetingId] = useState("");
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<GenerateStructuredSummaryOutput | null>(null);

  async function handleGenerate() {
    const trimmedText = text.trim();
    const trimmedMeetingId = meetingId.trim();

    if (!trimmedText && !trimmedMeetingId) {
      setError("Saisissez un identifiant de réunion ou un texte de transcription.");
      return;
    }

    setBusy(true);
    setError(null);

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

  return (
    <section className="structured-summary" aria-labelledby="structured-summary-title">
      <header className="structured-summary__header">
        <h2 id="structured-summary-title">Compte-rendu structuré</h2>
        <p>
          Collez une transcription ou indiquez l&apos;identifiant d&apos;une réunion existante,
          puis générez un compte-rendu avec synthèse, décisions et actions.
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
        <>
          <p className="structured-summary__meta">
            Réunion <span className="mono">{result.meetingId}</span>
          </p>
          <StructuredSummaryView summary={result.structured} showCopy headingLevel={3} />
        </>
      )}
    </section>
  );
}
