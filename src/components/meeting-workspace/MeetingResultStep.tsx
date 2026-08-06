import type { GenerateStructuredSummaryOutput } from "../../lib/ai/types";
import type { Transcription } from "../../lib/transcription";
import { StructuredSummaryView } from "../StructuredSummaryView";
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
          <StructuredSummaryView
            summary={summary.structured}
            providerId={summary.summary.providerId}
            headingLevel={3}
          />
        </section>
      )}
    </>
  );
}
