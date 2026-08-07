import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");

function readJson(relativePath: string): unknown {
  return JSON.parse(readFileSync(join(root, relativePath), "utf8"));
}

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
  const tauriConf = readJson("src-tauri/tauri.conf.json") as {
    build?: { removeUnusedCommands?: boolean };
    app: {
      security: {
        csp: unknown;
        assetProtocol?: { enable?: boolean; scope?: string[] };
      };
    };
  };

  const capabilities = readJson("src-tauri/capabilities/default.json") as {
    permissions: string[];
  };

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
  });

  it("active le protocole asset sur imports/ et recordings/", () => {
    const assetProtocol = tauriConf.app.security.assetProtocol;
    expect(assetProtocol?.enable).toBe(true);
    expect(assetProtocol?.scope).toEqual(
      expect.arrayContaining(["$APPDATA/imports/**", "$APPDATA/recordings/**"]),
    );
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
});
