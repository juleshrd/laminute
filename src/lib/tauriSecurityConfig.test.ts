import { describe, expect, it } from "vitest";
import tauriConf from "../../src-tauri/tauri.conf.json";
import capabilities from "../../src-tauri/capabilities/default.json";
import entitlements from "../../src-tauri/Entitlements.plist?raw";
import releaseWorkflow from "../../.github/workflows/release.yml?raw";

function serializeCsp(csp: unknown): string {
  if (typeof csp === "string") {
    return csp;
  }
  if (csp && typeof csp === "object") {
    return Object.entries(csp as Record<string, unknown>)
      .map(([directive, value]) => {
        const sources = Array.isArray(value) ? value.join(" ") : String(value);
        return `${directive} ${sources}`;
      })
      .join("; ");
  }
  return String(csp);
}

describe("configuration sécurité Tauri", () => {
  it("refuse csp: null et interdit unsafe-eval", () => {
    const { csp } = tauriConf.app.security;
    expect(csp).not.toBeNull();
    expect(csp).toBeTruthy();

    const serialized = serializeCsp(csp).toLowerCase();
    expect(serialized).not.toContain("unsafe-eval");
    expect(serialized).toContain("default-src");
    expect(serialized).toContain("'self'");
    expect(serialized).toMatch(/connect-src.*ipc:/);
    expect(serialized).toMatch(/media-src.*asset:/);
    expect(serialized).toMatch(/media-src.*http:\/\/asset\.localhost/);
  });

  it("confine asset: / asset.localhost à media-src uniquement", () => {
    const csp = tauriConf.app.security.csp;
    expect(csp).toBeTruthy();
    expect(typeof csp).toBe("object");

    const directives = csp as Record<string, unknown>;
    for (const [directive, value] of Object.entries(directives)) {
      const sources = Array.isArray(value) ? value.join(" ") : String(value);
      if (directive === "media-src") {
        expect(sources).toContain("asset:");
        expect(sources).toContain("http://asset.localhost");
        continue;
      }
      expect(sources).not.toMatch(/\basset:/);
      expect(sources).not.toContain("asset.localhost");
    }
  });

  it("active le protocole asset strictement sur imports/ et recordings/", () => {
    const assetProtocol = tauriConf.app.security.assetProtocol;
    expect(assetProtocol?.enable).toBe(true);

    const scope = assetProtocol?.scope;
    expect(Array.isArray(scope)).toBe(true);
    expect(scope).toEqual(["$APPDATA/imports/**/*", "$APPDATA/recordings/**/*"]);

    const patterns = scope as string[];
    for (const pattern of patterns) {
      expect(pattern).not.toMatch(/^\$HOME(\/|$)/);
      expect(pattern).not.toBe("**/*");
      expect(pattern).not.toBe("**");
      expect(pattern).not.toMatch(/\/\*\*$/);
    }
  });

  it("limite dialog à open/save et exclut opener", () => {
    const { permissions } = capabilities;

    expect(permissions).not.toContain("opener:default");
    expect(permissions.some((p) => p.startsWith("opener:"))).toBe(false);

    expect(permissions).not.toContain("dialog:default");
    expect(permissions).not.toContain("dialog:allow-message");
    expect(permissions).toContain("dialog:allow-open");
    expect(permissions).toContain("dialog:allow-save");

    expect(permissions).toContain("core:default");
    expect(permissions).toContain("updater:default");
    expect(permissions).toContain("process:allow-restart");
  });

  it("active removeUnusedCommands après stabilisation ACL", () => {
    expect(tauriConf.build?.removeUnusedCommands).toBe(true);
  });

  it("prépare l'entitlement microphone pour les builds macOS signés", () => {
    expect(tauriConf.bundle.macOS.entitlements).toBe("Entitlements.plist");
    expect(tauriConf.bundle.macOS.hardenedRuntime).toBe(true);

    expect(entitlements).toContain("com.apple.security.device.audio-input");
    expect(entitlements).toMatch(/com\.apple\.security\.device\.audio-input<\/key>\s*<true\/>/);
  });

  it("autorise un DMG non signé mais refuse une configuration Apple partielle", () => {
    expect(releaseWorkflow).toContain("Déterminer le mode de distribution macOS");
    expect(releaseWorkflow).toContain(
      "DMG macOS publié sans signature Developer ID ni notarisation",
    );
    expect(releaseWorkflow).toContain("enabled=false");
    expect(releaseWorkflow).toContain("mode=unsigned");
    expect(releaseWorkflow).toContain("Build and publish (macOS non signé)");
    expect(releaseWorkflow).toContain("Secrets Apple partiels ; configuration refusée");
    expect(releaseWorkflow).toContain("steps.apple_signing.outputs.enabled == 'true'");

    const unsignedBlock = releaseWorkflow.match(
      /- name: Build and publish \(macOS non signé\)([\s\S]*?)- name: Vérifier le bundle macOS/,
    )?.[1];
    expect(unsignedBlock).toBeTruthy();
    expect(unsignedBlock).not.toContain("APPLE_");
  });
});
