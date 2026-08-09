import { useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";

import type { GenerateStructuredSummaryOutput, ProviderInfo } from "../../lib/ai/types";
import type { Transcription } from "../../lib/transcription";
import { StructuredSummaryView } from "../StructuredSummaryView";
import "../StructuredSummaryPanel.css";

type ResultTab = "essential" | "transcript" | "audio";

interface MeetingResultStepProps {
  title?: string;
  transcription: Transcription | null;
  summary: GenerateStructuredSummaryOutput | null;
  audioPath?: string | null;
  providerName?: string;
  selectedProvider?: ProviderInfo | null;
}

export function MeetingResultStep({
  title = "Réunion",
  transcription,
  summary,
  audioPath = null,
  providerName,
  selectedProvider = null,
}: MeetingResultStepProps) {
  const [tab, setTab] = useState<ResultTab>("essential");
  const isLocal = selectedProvider?.capabilities.local ?? false;
  const structured = summary?.structured ?? null;

  return (
    <section className="meeting-result">
      <header className="meeting-result__head">
        <p className="lm-kicker">RÉSULTAT</p>
        <h2>{title || "Réunion"}</h2>
        <p className="today-view__lead">
          {isLocal
            ? `● Traité localement avec ${providerName ?? "Ollama"}`
            : `● Traité via ${providerName ?? "le cloud"}`}
        </p>
      </header>

      <div className="lm-tabs" role="tablist" aria-label="Contenu de la réunion">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "essential"}
          className={tab === "essential" ? "is-active" : undefined}
          onClick={() => setTab("essential")}
        >
          Essentiel
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "transcript"}
          className={tab === "transcript" ? "is-active" : undefined}
          onClick={() => setTab("transcript")}
        >
          Transcription
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "audio"}
          className={tab === "audio" ? "is-active" : undefined}
          onClick={() => setTab("audio")}
        >
          Audio
        </button>
      </div>

      {tab === "essential" ? (
        <div className="meeting-result__panel" role="tabpanel">
          {structured ? (
            <>
              <article className="essential-summary">
                <p className="lm-kicker">En une phrase</p>
                <p>{structured.synthese}</p>
              </article>
              <div className="essential-grid">
                <div>
                  <h3>Décisions</h3>
                  {structured.decisions.length > 0 ? (
                    structured.decisions.map((decision) => (
                      <article key={decision} className="essential-card">
                        <b>{decision}</b>
                      </article>
                    ))
                  ) : (
                    <p className="lm-subtle">Aucune décision identifiée.</p>
                  )}
                </div>
                <div>
                  <h3>Actions</h3>
                  {structured.actions.length > 0 ? (
                    structured.actions.map((action) => (
                      <article
                        key={`${action.titre}-${action.responsable ?? ""}`}
                        className="essential-card"
                      >
                        <b>{action.titre}</b>
                        {(action.responsable || action.echeance) && (
                          <span>
                            {[action.responsable, action.echeance].filter(Boolean).join(" · ")}
                          </span>
                        )}
                      </article>
                    ))
                  ) : (
                    <p className="lm-subtle">Aucune action identifiée.</p>
                  )}
                </div>
              </div>
              <details className="meeting-result__more">
                <summary>Voir le compte-rendu complet</summary>
                <StructuredSummaryView
                  summary={structured}
                  providerId={summary?.summary.providerId}
                  headingLevel={3}
                />
              </details>
            </>
          ) : (
            <p className="lm-subtle">Aucun compte-rendu structuré pour cette réunion.</p>
          )}
        </div>
      ) : null}

      {tab === "transcript" ? (
        <div className="meeting-result__panel" role="tabpanel">
          {transcription ? (
            <div className="transcription-result">
              <p>{transcription.content}</p>
              {transcription.language ? (
                <p className="meta">Langue détectée : {transcription.language}</p>
              ) : null}
            </div>
          ) : (
            <p className="lm-subtle">Aucune transcription disponible.</p>
          )}
        </div>
      ) : null}

      {tab === "audio" ? (
        <div className="meeting-result__panel" role="tabpanel">
          {audioPath ? (
            <audio controls src={convertFileSrc(audioPath)} className="meeting-detail__audio">
              Votre navigateur ne supporte pas la lecture audio.
            </audio>
          ) : (
            <p className="lm-subtle">Aucun fichier audio disponible.</p>
          )}
        </div>
      ) : null}
    </section>
  );
}
