import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MeetingIdleStep } from "./MeetingIdleStep";

describe("MeetingIdleStep", () => {
  afterEach(() => {
    cleanup();
  });

  it("affiche le CTA d'enregistrement et l'import MP3", () => {
    const onRequestStartRecording = vi.fn();
    const onPickMp3 = vi.fn();

    render(
      <MeetingIdleStep
        canStartRecording
        hasDevices
        importing={false}
        dragOver={false}
        onRequestStartRecording={onRequestStartRecording}
        onPickMp3={onPickMp3}
        onDragEnter={vi.fn()}
        onDragLeave={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Prêt à enregistrer ?" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Démarrer l'enregistrement" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Choisir un fichier MP3" })).toBeInTheDocument();
    expect(screen.getByText(/Importez un fichier MP3/i)).toBeInTheDocument();
    expect(screen.getByText(/100 Mo max \(limite transcription cloud\)/i)).toBeInTheDocument();
  });

  it("affiche un avertissement sans périphérique audio", () => {
    render(
      <MeetingIdleStep
        canStartRecording={false}
        hasDevices={false}
        importing={false}
        dragOver={false}
        onRequestStartRecording={vi.fn()}
        onPickMp3={vi.fn()}
        onDragEnter={vi.fn()}
        onDragLeave={vi.fn()}
      />,
    );

    expect(screen.getByText(/Aucun micro détecté/i)).toBeInTheDocument();
    expect(screen.getByText(/importer un MP3/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Démarrer l'enregistrement" })).toBeDisabled();
  });

  it("déclenche l'import MP3 au clic", () => {
    const onPickMp3 = vi.fn();

    render(
      <MeetingIdleStep
        canStartRecording
        hasDevices
        importing={false}
        dragOver={false}
        onRequestStartRecording={vi.fn()}
        onPickMp3={onPickMp3}
        onDragEnter={vi.fn()}
        onDragLeave={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Choisir un fichier MP3" }));
    expect(onPickMp3).toHaveBeenCalledOnce();
  });
});
