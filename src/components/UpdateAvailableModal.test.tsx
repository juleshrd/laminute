import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { UpdateAvailableModal } from "./UpdateAvailableModal";

describe("UpdateAvailableModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("affiche les versions et appelle onConfirm", () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();

    render(
      <UpdateAvailableModal
        currentVersion="0.1.0"
        nextVersion="0.2.0"
        notes="Correctifs"
        busy={false}
        progress={null}
        error={null}
        onConfirm={onConfirm}
        onCancel={onCancel}
      />,
    );

    expect(screen.getByRole("heading", { name: "Mise à jour disponible" })).toBeInTheDocument();
    expect(screen.getByText(/0\.2\.0/)).toBeInTheDocument();
    expect(screen.getByText(/0\.1\.0/)).toBeInTheDocument();
    expect(screen.getByText("Correctifs")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Mettre à jour" }));
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("appelle onCancel via Plus tard", () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();

    render(
      <UpdateAvailableModal
        currentVersion="0.1.0"
        nextVersion="0.2.0"
        busy={false}
        progress={null}
        error={null}
        onConfirm={onConfirm}
        onCancel={onCancel}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Plus tard" }));
    expect(onCancel).toHaveBeenCalledOnce();
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("désactive les actions pendant le téléchargement et affiche la progression", () => {
    render(
      <UpdateAvailableModal
        currentVersion="0.1.0"
        nextVersion="0.2.0"
        busy
        progress={{ downloaded: 50, contentLength: 100, finished: false }}
        error={null}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Mettre à jour" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Plus tard" })).toBeDisabled();
    expect(screen.getByRole("status")).toHaveTextContent("Téléchargement… 50 %");
  });

  it("affiche une erreur", () => {
    render(
      <UpdateAvailableModal
        currentVersion="0.1.0"
        nextVersion="0.2.0"
        busy={false}
        progress={null}
        error="Échec du téléchargement"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("Échec du téléchargement");
  });
});
