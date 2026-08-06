import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { MeetingResultStep } from "./MeetingResultStep";

describe("MeetingResultStep", () => {
  it("affiche la transcription et le compte-rendu structuré", () => {
    render(
      <MeetingResultStep
        transcription={{
          id: "tx-1",
          meetingId: "meeting-1",
          content: "Bonjour à tous.",
          language: "fr",
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
        }}
        summary={{
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

    expect(screen.getByRole("heading", { name: "Transcription" })).toBeInTheDocument();
    expect(screen.getByText("Bonjour à tous.")).toBeInTheDocument();
    expect(screen.getByText(/Langue détectée : fr/)).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Compte-rendu structuré" })).toBeInTheDocument();
    expect(screen.getByText("Réunion productive.")).toBeInTheDocument();
    expect(screen.getByText("Valider le MVP")).toBeInTheDocument();
    expect(screen.getByText("Préparer la démo")).toBeInTheDocument();
  });
});
