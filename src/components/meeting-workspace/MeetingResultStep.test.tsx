import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { MeetingResultStep } from "./MeetingResultStep";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => path,
}));

describe("MeetingResultStep", () => {
  it("priorise l'essentiel puis expose transcription et audio", () => {
    render(
      <MeetingResultStep
        title="Comité produit"
        transcription={{
          id: "tx-1",
          meetingId: "meeting-1",
          content: "Bonjour à tous.",
          language: "fr",
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
        }}
        audioPath="/tmp/rec.wav"
        summary={{
          jobId: "summary-1",
          meetingId: "meeting-1",
          summary: {
            id: "summary-1",
            meetingId: "meeting-1",
            content: "{}",
            createdAt: "2026-01-01T00:00:00Z",
            updatedAt: "2026-01-01T00:00:00Z",
          },
          structured: {
            synthese: "Réunion productive.",
            decisions: ["Valider le MVP"],
            actions: [
              {
                titre: "Préparer la démo",
                responsable: "Alice",
                echeance: "vendredi",
              },
            ],
            risques: [],
            questionsOuvertes: [],
          },
          actions: [],
        }}
      />,
    );

    expect(screen.getByRole("tab", { name: "Essentiel" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("En une phrase").closest("article")).toHaveTextContent(
      "Réunion productive.",
    );
    expect(screen.getAllByText("Valider le MVP").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Préparer la démo").length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("tab", { name: "Transcription" }));
    expect(screen.getByText("Bonjour à tous.")).toBeInTheDocument();
    expect(screen.getByText(/Langue détectée : fr/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "Audio" }));
    expect(document.querySelector("audio")).not.toBeNull();
  });
});
