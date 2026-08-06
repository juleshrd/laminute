import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MeetingReadyStep } from "./MeetingReadyStep";

const defaultProps = {
  flowPhase: "ready" as const,
  title: "Comité produit",
  meetingId: "meeting-1",
  filePath: "/tmp/import.mp3",
  durationSecs: 120,
  hasApiKey: true,
  providerName: "Mistral AI",
  selectedProvider: {
    id: "mistral",
    displayName: "Mistral AI",
    capabilities: {
      transcription: true,
      summary: true,
      local: false,
      streaming: false,
      diarization: true,
    },
  },
  isSummarizeOnly: false,
  isBusy: false,
  pastedText: "",
  onTitleChange: vi.fn(),
  onTitleBlur: vi.fn(),
  onPastedTextChange: vi.fn(),
  onProcess: vi.fn(),
  onSummarizeFromText: vi.fn(),
};

describe("MeetingReadyStep", () => {
  afterEach(() => {
    cleanup();
  });

  it("affiche les détails de la réunion et le bouton Traiter", () => {
    render(<MeetingReadyStep {...defaultProps} />);

    expect(screen.getByLabelText("Titre")).toHaveValue("Comité produit");
    expect(screen.getByText("/tmp/import.mp3")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Traiter" })).toBeEnabled();
  });

  it("affiche Réessayer en phase error", () => {
    render(<MeetingReadyStep {...defaultProps} flowPhase="error" />);
    expect(screen.getByRole("button", { name: "Réessayer" })).toBeInTheDocument();
  });

  it("désactive Traiter sans clé API", () => {
    render(<MeetingReadyStep {...defaultProps} hasApiKey={false} />);
    expect(screen.getByText(/Configurez .* dans les réglages IA/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Traiter" })).toBeDisabled();
  });

  it("affiche le collage de texte en mode summarize-only", () => {
    const onSummarizeFromText = vi.fn();

    render(
      <MeetingReadyStep
        {...defaultProps}
        isSummarizeOnly
        selectedProvider={{
          ...defaultProps.selectedProvider,
          capabilities: { ...defaultProps.selectedProvider.capabilities, transcription: false },
        }}
        pastedText="Notes de réunion"
        onSummarizeFromText={onSummarizeFromText}
      />,
    );

    expect(screen.getByLabelText("Texte de la réunion")).toHaveValue("Notes de réunion");
    fireEvent.click(screen.getByRole("button", { name: "Générer le compte-rendu" }));
    expect(onSummarizeFromText).toHaveBeenCalledOnce();
  });
});
