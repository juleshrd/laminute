import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { MeetingRecordingStep } from "./MeetingRecordingStep";

describe("MeetingRecordingStep", () => {
  it("affiche le chrono et le bouton Terminer la réunion", () => {
    const onStopRecording = vi.fn();

    render(<MeetingRecordingStep durationSecs={72} onStopRecording={onStopRecording} />);

    expect(screen.getByLabelText("Durée de l'enregistrement")).toHaveTextContent("1:12");
    fireEvent.click(screen.getByRole("button", { name: "Terminer la réunion" }));
    expect(onStopRecording).toHaveBeenCalledOnce();
  });
});
