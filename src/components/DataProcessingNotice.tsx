interface DataProcessingNoticeProps {
  providerName: string;
  capabilities?: {
    transcription: boolean;
    summary: boolean;
    local: boolean;
  };
}

export function DataProcessingNotice({ providerName, capabilities }: DataProcessingNoticeProps) {
  const isLocal = capabilities?.local ?? false;
  const hasTranscription = capabilities?.transcription ?? true;

  if (isLocal) {
    return (
      <p className="data-processing-notice" role="note">
        <strong>Traitement local via {providerName} :</strong> le compte-rendu est généré sur votre
        machine via Ollama. Aucune donnée n&apos;est envoyée à un service cloud. La transcription
        audio n&apos;est pas disponible avec ce fournisseur.
      </p>
    );
  }

  return (
    <p className="data-processing-notice" role="note">
      <strong>Données envoyées à {providerName} :</strong>
      {hasTranscription
        ? " la transcription transmet le fichier audio ; le compte-rendu transmet uniquement le texte transcrit."
        : " le compte-rendu transmet uniquement le texte fourni."}{" "}
      Aucune clé API n&apos;est incluse dans ces échanges.
    </p>
  );
}
