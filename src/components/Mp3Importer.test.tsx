import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { Mp3Importer } from "./Mp3Importer";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

describe("Mp3Importer", () => {
  it("affiche les instructions d'import MP3", () => {
    render(<Mp3Importer />);

    expect(screen.getByRole("heading", { name: "Import MP3" })).toBeInTheDocument();
    expect(
      screen.getByText(/Glissez-déposez un fichier MP3 ici/i),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Choisir un fichier MP3" })).toBeInTheDocument();
    expect(screen.getByText(/MP3 uniquement · 500 Mo max/i)).toBeInTheDocument();
  });
});
