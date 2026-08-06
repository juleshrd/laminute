import type { GenerateStructuredSummaryOutput } from "../../lib/ai/types";
import type { Transcription } from "../../lib/transcription";
import "../StructuredSummaryPanel.css";

interface MeetingResultStepProps {
  transcription: Transcription | null;
  summary: GenerateStructuredSummaryOutput | null;
}

export function MeetingResultStep({ transcription, summary }: MeetingResultStepProps) {
  return (
    <>
      {transcription && (
        <section className="panel">
          <h2>Transcription</h2>
          <div className="transcription-result">
            <p>{transcription.content}</p>
            {transcription.language && (
              <p className="meta">Langue détectée : {transcription.language}</p>
            )}
          </div>
        </section>
      )}

      {summary && (
        <section className="panel structured-summary-inline">
          <h2>Compte-rendu structuré</h2>

          <article className="structured-summary__block">
            <h3>Synthèse</h3>
            <p>{summary.structured.synthese}</p>
          </article>

          <article className="structured-summary__block">
            <h3>Décisions</h3>
            {summary.structured.decisions.length > 0 ? (
              <ul>
                {summary.structured.decisions.map((decision) => (
                  <li key={decision}>{decision}</li>
                ))}
              </ul>
            ) : (
              <p className="structured-summary__empty">Aucune décision identifiée.</p>
            )}
          </article>

          <article className="structured-summary__block">
            <h3>Actions</h3>
            {summary.structured.actions.length > 0 ? (
              <ul>
                {summary.structured.actions.map((action) => (
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
        </section>
      )}
    </>
  );
}
