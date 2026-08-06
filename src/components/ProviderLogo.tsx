import mistralLogo from "../assets/providers/mistral.svg";
import ollamaLogo from "../assets/providers/ollama.svg";
import openaiLogo from "../assets/providers/openai.svg";

const LOGOS: Record<string, string> = {
  mistral: mistralLogo,
  openai: openaiLogo,
  ollama: ollamaLogo,
};

interface ProviderLogoProps {
  providerId: string;
  displayName: string;
  className?: string;
}

export function ProviderLogo({ providerId, displayName, className }: ProviderLogoProps) {
  const src = LOGOS[providerId];
  if (!src) {
    return <span className={`lm-provider-logo lm-provider-logo--fallback ${className ?? ""}`} />;
  }

  return (
    <img
      src={src}
      alt=""
      className={`lm-provider-logo ${className ?? ""}`}
      width={28}
      height={28}
      aria-hidden="true"
      data-provider={providerId}
      title={displayName}
    />
  );
}
