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
  /** Taille visuelle ; `lg` pour les cartes onboarding. */
  size?: "sm" | "md" | "lg";
}

const SIZE_PX: Record<NonNullable<ProviderLogoProps["size"]>, number> = {
  sm: 22,
  md: 28,
  lg: 36,
};

export function ProviderLogo({
  providerId,
  displayName,
  className,
  size = "md",
}: ProviderLogoProps) {
  const src = LOGOS[providerId];
  const px = SIZE_PX[size];
  const sizeClass = `lm-provider-logo--${size}`;

  if (!src) {
    return (
      <span
        className={`lm-provider-logo lm-provider-logo--fallback ${sizeClass} ${className ?? ""}`}
        data-provider={providerId}
        title={displayName}
        aria-hidden="true"
      />
    );
  }

  return (
    <img
      src={src}
      alt=""
      className={`lm-provider-logo ${sizeClass} ${className ?? ""}`}
      width={px}
      height={px}
      aria-hidden="true"
      data-provider={providerId}
      title={displayName}
    />
  );
}

/** Identifiants pour lesquels un logo SVG est embarqué. */
export function hasProviderLogo(providerId: string): boolean {
  return providerId in LOGOS;
}
