import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { LocalStorageInfo } from "../lib/privacy";
import { StorageLocationControl } from "./StorageLocationControl";

const chooseStorageParent = vi.fn();
const prepareLocalStorageChange = vi.fn();
const applyLocalStorageChange = vi.fn();

vi.mock("../lib/privacy", () => ({
  chooseStorageParent: (...args: unknown[]) => chooseStorageParent(...args),
  prepareLocalStorageChange: (...args: unknown[]) => prepareLocalStorageChange(...args),
  applyLocalStorageChange: (...args: unknown[]) => applyLocalStorageChange(...args),
  formatBytes: (bytes: number) => `${bytes} o`,
}));

const STORAGE: LocalStorageInfo = {
  meetingsCount: 2,
  rootDir: "/Users/test/Library/Application Support/app.laminute.desktop",
  defaultRootDir: "/Users/test/Library/Application Support/app.laminute.desktop",
  isCustom: false,
  dbPath: "/Users/test/Library/Application Support/app.laminute.desktop/laminute.db",
  importsDir: "/Users/test/Library/Application Support/app.laminute.desktop/imports",
  recordingsDir: "/Users/test/Library/Application Support/app.laminute.desktop/recordings",
  availableBytes: 900_000_000,
};

describe("StorageLocationControl", () => {
  beforeEach(() => {
    chooseStorageParent.mockResolvedValue("/Volumes/Travail");
    prepareLocalStorageChange.mockResolvedValue({
      token: "grant-1",
      currentPath: STORAGE.rootDir,
      destinationPath: "/Volumes/Travail/La Minute",
      dataBytes: 42_000,
      availableBytes: 900_000_000,
      isDefault: false,
    });
    applyLocalStorageChange.mockResolvedValue({
      rootDir: "/Volumes/Travail/La Minute",
      movedBytes: 42_000,
      sourceCleanupWarning: null,
    });
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("prévisualise puis applique une migration confirmée", async () => {
    const onStorageChanged = vi.fn();
    render(<StorageLocationControl storage={STORAGE} onStorageChanged={onStorageChanged} />);

    fireEvent.click(screen.getByRole("button", { name: "Choisir un autre emplacement" }));

    expect(
      await screen.findByRole("heading", { name: "Confirmer le déplacement" }),
    ).toBeInTheDocument();
    expect(screen.getByText("/Volumes/Travail/La Minute")).toBeInTheDocument();
    expect(prepareLocalStorageChange).toHaveBeenCalledWith("/Volumes/Travail", false);

    fireEvent.click(screen.getByRole("button", { name: "Déplacer les données" }));
    await waitFor(() => {
      expect(applyLocalStorageChange).toHaveBeenCalledWith("grant-1");
      expect(onStorageChanged).toHaveBeenCalledOnce();
    });
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Stockage déplacé vers /Volumes/Travail/La Minute",
    );
  });

  it("affiche une erreur bloquante si le dossier ne peut pas être validé", async () => {
    prepareLocalStorageChange.mockRejectedValueOnce(new Error("Espace insuffisant"));
    render(<StorageLocationControl storage={STORAGE} onStorageChanged={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Choisir un autre emplacement" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Espace insuffisant");
    expect(screen.queryByRole("button", { name: "Déplacer les données" })).not.toBeInTheDocument();
  });
});
