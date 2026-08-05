import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { RecordingConsentModal } from "./RecordingConsentModal";

describe("RecordingConsentModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("appelle onConfirm quand l'utilisateur confirme", () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();

    render(<RecordingConsentModal onConfirm={onConfirm} onCancel={onCancel} />);

    fireEvent.click(
      screen.getByRole("button", { name: /J'ai informé les participants/i }),
    );

    expect(onConfirm).toHaveBeenCalledOnce();
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("appelle onCancel quand l'utilisateur annule", () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();

    render(<RecordingConsentModal onConfirm={onConfirm} onCancel={onCancel} />);

    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));

    expect(onCancel).toHaveBeenCalledOnce();
    expect(onConfirm).not.toHaveBeenCalled();
  });
});
