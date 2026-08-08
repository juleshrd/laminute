/** Détecte une URL Ollama loopback pour l'UI (la validation autoritaire est côté Rust). */
export function isOllamaLoopbackUrl(raw: string): boolean {
  const trimmed = raw.trim();
  if (!trimmed) {
    return false;
  }
  try {
    const url = new URL(trimmed);
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      return false;
    }
    // Node/jsdom peut renvoyer « [::1] » ; les navigateurs renvoient « ::1 ».
    const host = url.hostname.toLowerCase().replace(/^\[|\]$/g, "");
    return host === "localhost" || host === "127.0.0.1" || host === "::1";
  } catch {
    return false;
  }
}
