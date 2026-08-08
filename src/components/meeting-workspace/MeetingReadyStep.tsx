import type { ProviderInfo } from "../../lib/ai/types";
import { formatDuration } from "../../lib/audio";
import type { MeetingFlowPhase } from "../../lib/meetingFlow";
import { DataProcessingNotice } from "../DataProcessingNotice";

interface MeetingReadyStepProps {
  flowPhase: MeetingFlowPhase;
  title: string;
  meetingId: string | null;
  filePath: string;
  durationSecs: number | null;
  hasApiKey: boolean;
  providerName: string;
  selectedProvider: ProviderInfo | null;
  ollamaBaseUrl: string | null;
  isSummarizeOnly: boolean;
  isBusy: boolean;
  pastedText: string;
  onTitleChange: (title: string) => void;
  onTitleBlur: () => void;
  onPastedTextChange: (text: string) => void;
  onProcess: () => void;
  onSummarizeFromText: () => void;
}

export function MeetingReadyStep({
  flowPhase,
  title,
  meetingId,
  filePath,
  durationSecs,
  hasApiKey,
  providerName,
  selectedProvider,
  ollamaBaseUrl,
  isSummarizeOnly,
  isBusy,
  pastedText,
  onTitleChange,
  onTitleBlur,
  onPastedTextChange,
  onProcess,
  onSummarizeFromText,
}: MeetingReadyStepProps) {
  const showActionControls = flowPhase === "ready" || flowPhase === "error";

  return (
    <section className="panel">
      <h2>Réunion</h2>
      <div className="meeting-workspace__field">
        <label htmlFor="meeting-title">Titre</label>
        <input
          id="meeting-title"
          type="text"
          value={title}
          onChange={(event) => onTitleChange(event.target.value)}
          onBlur={() => void onTitleBlur()}
        />
      </div>

      <dl className="status-grid">
        <div>
          <dt>Durée</dt>
          <dd>{formatDuration(durationSecs)}</dd>
        </div>
        {meetingId && (
          <div>
            <dt>Identifiant</dt>
            <dd className="mono">{meetingId}</dd>
          </div>
        )}
        <div>
          <dt>Fichier</dt>
          <dd className="mono">{filePath}</dd>
        </div>
      </dl>

      {!hasApiKey && showActionControls && (
        <p className="warning">
          Configurez {isSummarizeOnly ? "la connexion" : "une clé API"} pour {providerName} dans les
          réglages IA avant de traiter la réunion.
        </p>
      )}

      {showActionControls && hasApiKey && (
        <DataProcessingNotice
          providerId={selectedProvider?.id}
          providerName={providerName}
          ollamaBaseUrl={ollamaBaseUrl}
          capabilities={selectedProvider?.capabilities}
        />
      )}

      {isSummarizeOnly && showActionControls && (
        <>
          <p className="warning" role="note">
            {providerName} ne prend pas en charge la transcription audio. Collez le texte de la
            réunion ci-dessous, ou choisissez OpenAI ou Mistral pour transcrire automatiquement.
          </p>
          <div className="meeting-workspace__field">
            <label htmlFor="pasted-transcript">Texte de la réunion</label>
            <textarea
              id="pasted-transcript"
              rows={8}
              value={pastedText}
              disabled={isBusy}
              onChange={(event) => onPastedTextChange(event.target.value)}
              placeholder="Collez ici la transcription ou les notes de la réunion…"
            />
          </div>
          <div className="row controls">
            <button
              type="button"
              onClick={() => void onSummarizeFromText()}
              disabled={!hasApiKey || isBusy || !pastedText.trim()}
            >
              Générer le compte-rendu
            </button>
          </div>
        </>
      )}

      {!isSummarizeOnly && showActionControls && (
        <div className="row controls">
          <button
            type="button"
            onClick={() => void onProcess()}
            disabled={!hasApiKey || isBusy || !title.trim()}
          >
            {flowPhase === "error" ? "Réessayer" : "Traiter"}
          </button>
        </div>
      )}
    </section>
  );
}
