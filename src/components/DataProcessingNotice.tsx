interface DataProcessingNoticeProps {
  providerName: string;
}

export function DataProcessingNotice({ providerName }: DataProcessingNoticeProps) {
  return (
    <p className="data-processing-notice" role="note">
      <strong>Données envoyées à {providerName} :</strong> la transcription transmet le fichier
      audio ; le compte-rendu transmet uniquement le texte transcrit. Aucune clé API n&apos;est
      incluse dans ces échanges.
    </p>
  );
}
