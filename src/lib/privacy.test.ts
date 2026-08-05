import { describe, expect, it } from "vitest";

import { buildExportFilename, formatBytes } from "./privacy";

describe("buildExportFilename", () => {
  it("produit un nom de fichier sûr à partir du titre", () => {
    const name = buildExportFilename("Comité produit #1", "2026-08-05T12:00:00.000Z");
    expect(name).toBe("laminute-Comite-produit-1-2026-08-05.json");
  });

  it("utilise un nom par défaut si le titre est vide", () => {
    const name = buildExportFilename("!!!", "2026-08-05T12:00:00.000Z");
    expect(name).toBe("laminute-reunion-2026-08-05.json");
  });
});

describe("formatBytes", () => {
  it("formate les tailles lisibles", () => {
    expect(formatBytes(512)).toBe("512 o");
    expect(formatBytes(2048)).toBe("2.0 Ko");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 Mo");
    expect(formatBytes(null)).toBe("—");
  });
});
