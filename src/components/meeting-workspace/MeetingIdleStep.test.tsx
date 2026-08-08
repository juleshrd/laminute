import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MeetingIdleStep } from "./MeetingIdleStep";

const baseProps = {
  canStartRecording: true,
  hasDevices: true,
  importing: false,
  dragOver: false,
  deviceName: "Micro intégré",
  devices: [{ id: "mic-1", name: "Micro intégré" }],
  selectedDeviceId: "mic-1",
  onSelectDevice: vi.fn(),
  onRequestStartRecording: vi.fn(),
  onPickMp3: vi.fn(),
  onDragEnter: vi.fn(),
  onDragLeave: vi.fn(),
};

describe("MeetingIdleStep", () => {
  afterEach(() => {
    cleanup();
  });

  it("affiche le CTA d'enregistrement et l'import", () => {
    const onRequestStartRecording = vi.fn();
    const onPickMp3 = vi.fn();

    render(
      <MeetingIdleStep
        {...baseProps}
        onRequestStartRecording={onRequestStartRecording}
        onPickMp3={onPickMp3}
      />,
    );

    expect(screen.getByRole("heading", { name: "Bonjour." })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Démarrer l'enregistrement" })).toBeEnabled();
    expect(
      screen.getByRole("button", { name: /Importer un enregistrement/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(/MP3, M4A ou WAV/i)).toBeInTheDocument();
  });

  it("affiche un avertissement sans périphérique audio", () => {
    render(
      <MeetingIdleStep
        {...baseProps}
        canStartRecording={false}
        hasDevices={false}
        deviceName={null}
        devices={[]}
        selectedDeviceId=""
      />,
    );

    expect(screen.getByText(/Aucun périphérique d'entrée audio détecté/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Démarrer l'enregistrement" })).toBeDisabled();
  });

  it("déclenche l'import au clic", () => {
    const onPickMp3 = vi.fn();

    render(<MeetingIdleStep {...baseProps} onPickMp3={onPickMp3} />);

    fireEvent.click(screen.getByRole("button", { name: /Importer un enregistrement/i }));
    expect(onPickMp3).toHaveBeenCalledOnce();
  });
});
