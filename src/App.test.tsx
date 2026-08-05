import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import App from "./App";

vi.mock("./lib/updater", () => ({
  checkForAppUpdate: vi.fn().mockResolvedValue(null),
  applyAppUpdate: vi.fn(),
}));

vi.mock("./components/MeetingWorkspace", () => ({
  MeetingWorkspace: () => (
    <div data-testid="workspace-view">
      <label>
        Titre de réunion test
        <input aria-label="Titre de réunion test" />
      </label>
    </div>
  ),
}));

vi.mock("./components/MeetingHistory", () => ({
  MeetingHistory: () => <div data-testid="history-view">Historique test</div>,
}));

vi.mock("./components/AiProviderSettings", () => ({
  AiProviderSettings: () => <div>Fournisseurs test</div>,
}));

vi.mock("./components/PrivacySettings", () => ({
  PrivacySettings: () => <div>Confidentialité test</div>,
}));

vi.mock("./components/UpdateAvailableModal", () => ({
  UpdateAvailableModal: () => <div>Modal de mise à jour test</div>,
}));

afterEach(cleanup);

describe("App", () => {
  it("n'affiche que la vue choisie dans la navigation principale", () => {
    render(<App />);

    expect(screen.getByTestId("workspace-view")).toBeVisible();
    expect(screen.queryByTestId("history-view")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Réunion" })).toHaveAttribute("aria-current", "page");

    fireEvent.click(screen.getByRole("button", { name: "Historique" }));

    expect(screen.getByTestId("history-view")).toBeVisible();
    expect(screen.getByTestId("workspace-view")).not.toBeVisible();
    expect(screen.getByRole("button", { name: "Historique" })).toHaveAttribute(
      "aria-current",
      "page",
    );

    fireEvent.click(screen.getByRole("button", { name: "Réglages" }));

    expect(screen.getByText("Fournisseurs test")).toBeVisible();
    expect(screen.getByText("Confidentialité test")).toBeVisible();
    expect(screen.getByTestId("history-view")).not.toBeVisible();
  });

  it("préserve la réunion courante lors d'un changement de vue", () => {
    render(<App />);
    const title = screen.getByLabelText("Titre de réunion test");
    fireEvent.change(title, { target: { value: "Comité produit" } });

    fireEvent.click(screen.getByRole("button", { name: "Historique" }));
    fireEvent.click(screen.getByRole("button", { name: "Réunion" }));

    expect(screen.getByLabelText("Titre de réunion test")).toHaveValue("Comité produit");
  });
});
