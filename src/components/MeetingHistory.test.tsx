import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { MeetingHistory } from "./MeetingHistory";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  convertFileSrc: (path: string) => path,
}));

describe("MeetingHistory", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_ai_providers") {
        return Promise.resolve([
          { id: "mistral", displayName: "Mistral AI", capabilities: {} },
        ]);
      }
      if (command === "search_meetings") {
        return Promise.resolve([
          {
            id: "m-1",
            title: "Comité produit",
            status: "completed",
            startedAt: "2026-08-05T10:00:00Z",
            endedAt: null,
            createdAt: "2026-08-05T10:00:00Z",
            updatedAt: "2026-08-05T11:00:00Z",
            snippet: "Réunion productive",
          },
        ]);
      }
      return Promise.resolve([]);
    });
  });

  it("affiche les résultats de recherche", async () => {
    render(<MeetingHistory />);

    expect(await screen.findByText("Comité produit")).toBeInTheDocument();
    expect(screen.getByText("Réunion productive")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith(
      "search_meetings",
      expect.objectContaining({ filters: expect.any(Object) }),
    );
  });

  it("ouvre le détail au clic sur une réunion", async () => {

    invokeMock.mockImplementation((command: string) => {
      if (command === "list_ai_providers") {
        return Promise.resolve([]);
      }
      if (command === "search_meetings") {
        return Promise.resolve([
          {
            id: "m-1",
            title: "Comité produit",
            status: "completed",
            startedAt: "2026-08-05T10:00:00Z",
            endedAt: null,
            createdAt: "2026-08-05T10:00:00Z",
            updatedAt: "2026-08-05T11:00:00Z",
          },
        ]);
      }
      if (command === "get_meeting") {
        return Promise.resolve({
          id: "m-1",
          title: "Comité produit",
          status: "completed",
          startedAt: "2026-08-05T10:00:00Z",
          endedAt: null,
          createdAt: "2026-08-05T10:00:00Z",
          updatedAt: "2026-08-05T11:00:00Z",
          description: null,
          audioFiles: [],
          transcriptions: [{ id: "t-1", meetingId: "m-1", content: "Bonjour", createdAt: "", updatedAt: "" }],
          summaries: [],
          actions: [],
        });
      }
      return Promise.resolve([]);
    });

    render(<MeetingHistory />);
    await screen.findByText("Comité produit");

    const title = screen.getAllByText("Comité produit")[0];
    const itemButton = title.closest("button");
    expect(itemButton).not.toBeNull();
    fireEvent.click(itemButton!);

    await waitFor(() => {
      expect(screen.getByText("← Retour à la liste")).toBeInTheDocument();
      expect(screen.getByText("Bonjour")).toBeInTheDocument();
    });
  });
});
