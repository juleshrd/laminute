import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { MeetingProcessingStep } from "./MeetingProcessingStep";

describe("MeetingProcessingStep", () => {
  it("affiche le message de progression", () => {
    render(<MeetingProcessingStep progressMessage="Transcription en cours…" />);
    expect(screen.getByText("Transcription en cours…")).toBeInTheDocument();
  });
});
