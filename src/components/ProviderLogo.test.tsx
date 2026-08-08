import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";

import { hasProviderLogo, ProviderLogo } from "./ProviderLogo";

describe("ProviderLogo", () => {
  it.each(["mistral", "openai", "ollama"] as const)("embarque un SVG pour %s", (providerId) => {
    expect(hasProviderLogo(providerId)).toBe(true);
    const { container } = render(<ProviderLogo providerId={providerId} displayName={providerId} />);
    const img = container.querySelector(`img[data-provider="${providerId}"]`);
    expect(img).toBeTruthy();
    expect(img?.getAttribute("src") ?? "").toMatch(/(\.svg|image\/svg\+xml)/);
  });

  it("affiche un fallback pour un fournisseur inconnu", () => {
    const { container } = render(<ProviderLogo providerId="unknown" displayName="Inconnu" />);
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector(".lm-provider-logo--fallback")).toBeTruthy();
  });
});
