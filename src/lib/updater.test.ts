import { describe, expect, it, vi, beforeEach } from "vitest";

const checkMock = vi.hoisted(() => vi.fn());
const relaunchMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: checkMock,
}));

vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: relaunchMock,
}));

import {
  applyAppUpdate,
  checkForAppUpdate,
  describeUpdateCheckError,
  formatUpdateProgress,
  probeAppUpdate,
} from "./updater";

describe("updater", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("checkForAppUpdate délègue à check()", async () => {
    const update = { version: "0.2.0" };
    checkMock.mockResolvedValueOnce(update);

    await expect(checkForAppUpdate()).resolves.toBe(update);
    expect(checkMock).toHaveBeenCalledOnce();
  });

  it("checkForAppUpdate renvoie null quand aucune mise à jour", async () => {
    checkMock.mockResolvedValueOnce(null);

    await expect(checkForAppUpdate()).resolves.toBeNull();
  });

  it("checkForAppUpdate propage les erreurs réseau", async () => {
    checkMock.mockRejectedValueOnce(new Error("network down"));

    await expect(checkForAppUpdate()).rejects.toThrow("network down");
  });

  it("probeAppUpdate distingue update / à jour / erreur", async () => {
    const update = { version: "0.2.0" };
    checkMock.mockResolvedValueOnce(update);
    await expect(probeAppUpdate()).resolves.toEqual({ status: "available", update });

    checkMock.mockResolvedValueOnce(null);
    await expect(probeAppUpdate()).resolves.toEqual({ status: "up-to-date" });

    checkMock.mockRejectedValueOnce(new Error("timeout"));
    await expect(probeAppUpdate()).resolves.toEqual({
      status: "error",
      message: "Vérification des mises à jour impossible : timeout",
    });
  });

  it("describeUpdateCheckError fournit un message par défaut", () => {
    expect(describeUpdateCheckError("x")).toBe(
      "Vérification des mises à jour impossible (réseau indisponible ou flux inaccessible).",
    );
  });

  it("applyAppUpdate télécharge, installe puis relance", async () => {
    const downloadAndInstall = vi.fn(async (onEvent?: (event: unknown) => void) => {
      onEvent?.({ event: "Started", data: { contentLength: 100 } });
      onEvent?.({ event: "Progress", data: { chunkLength: 40 } });
      onEvent?.({ event: "Progress", data: { chunkLength: 60 } });
      onEvent?.({ event: "Finished" });
    });
    relaunchMock.mockResolvedValueOnce(undefined);

    const onProgress = vi.fn();
    await applyAppUpdate({ downloadAndInstall } as never, onProgress);

    expect(downloadAndInstall).toHaveBeenCalledOnce();
    expect(relaunchMock).toHaveBeenCalledOnce();
    expect(onProgress).toHaveBeenCalledWith({
      downloaded: 100,
      contentLength: 100,
      finished: true,
    });
  });

  it("formatUpdateProgress affiche le pourcentage quand la taille est connue", () => {
    expect(formatUpdateProgress({ downloaded: 50, contentLength: 100, finished: false })).toBe(
      "Téléchargement… 50 %",
    );
    expect(formatUpdateProgress({ downloaded: 100, contentLength: 100, finished: true })).toBe(
      "Installation en cours…",
    );
  });
});
