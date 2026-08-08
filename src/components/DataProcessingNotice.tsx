import { buildDataProcessingNotice, type PrivacyProviderId } from "../content/privacyNotices";

interface DataProcessingNoticeProps {
  providerId?: PrivacyProviderId;
  providerName: string;
  ollamaBaseUrl?: string | null;
  capabilities?: {
    transcription: boolean;
    summary: boolean;
    local: boolean;
  };
}

export function DataProcessingNotice({
  providerId,
  providerName,
  ollamaBaseUrl,
  capabilities,
}: DataProcessingNoticeProps) {
  const resolvedId = providerId ?? (capabilities?.local ? "ollama" : "mistral");
  const notice = buildDataProcessingNotice({
    providerId: resolvedId,
    providerName,
    ollamaBaseUrl,
    capabilities,
  });

  return (
    <p className="data-processing-notice" role="note">
      <strong>{notice.title} :</strong> {notice.body}
    </p>
  );
}
