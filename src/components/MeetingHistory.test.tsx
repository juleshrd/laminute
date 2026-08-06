import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

import { MeetingHistory } from "./MeetingHistory";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  convertFileSrc: (path: string) => path,
}));

describe("MeetingHistory", () => {
  beforeEach(() => {
    cleanup();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
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
            snippet: null,
          },
        ]);
      }
      return Promise.resolve([]);
    });
  });

  afterEach(() => {
    cleanup();
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  async function flushSearchDebounce() {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
  }

  it("affiche les réunions de la liste", async () => {
    render(<MeetingHistory />);
    await flushSearchDebounce();

    expect(await screen.findByText("Comité produit")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("search_meetings", { filters: {} });
  });

  it("recherche via search_meetings et affiche l'extrait", async () => {
    invokeMock.mockImplementation((command: string, args?: { filters?: { query?: string } }) => {
      if (command === "search_meetings") {
        const q = args?.filters?.query;
        if (q === "Dufour") {
          return Promise.resolve([
            {
              id: "m-2",
              title: "Point commercial",
              status: "completed",
              startedAt: "2026-08-04T10:00:00Z",
              endedAt: null,
              createdAt: "2026-08-04T10:00:00Z",
              updatedAt: "2026-08-04T11:00:00Z",
              snippet: "…Discussion avec le client Dufour sur…",
            },
          ]);
        }
        return Promise.resolve([
          {
            id: "m-1",
            title: "Comité produit",
            status: "completed",
            startedAt: "2026-08-05T10:00:00Z",
            endedAt: null,
            createdAt: "2026-08-05T10:00:00Z",
            updatedAt: "2026-08-05T11:00:00Z",
            snippet: null,
          },
        ]);
      }
      return Promise.resolve([]);
    });

    render(<MeetingHistory />);
    await flushSearchDebounce();
    expect(screen.getByRole("heading", { name: "Comité produit" })).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("Rechercher une réunion…"), {
      target: { value: "Dufour" },
    });
    await flushSearchDebounce();

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("search_meetings", {
        filters: { query: "Dufour" },
      });
    });
    expect(screen.getByRole("heading", { name: "Point commercial" })).toBeInTheDocument();
    expect(screen.getByText(/client Dufour/)).toBeInTheDocument();
  });

  it("affiche un état clair sans résultat", async () => {
    invokeMock.mockImplementation((command: string, args?: { filters?: { query?: string } }) => {
      if (command === "search_meetings") {
        if (args?.filters?.query) {
          return Promise.resolve([]);
        }
        return Promise.resolve([
          {
            id: "m-1",
            title: "Comité produit",
            status: "completed",
            startedAt: "2026-08-05T10:00:00Z",
            endedAt: null,
            createdAt: "2026-08-05T10:00:00Z",
            updatedAt: "2026-08-05T11:00:00Z",
            snippet: null,
          },
        ]);
      }
      return Promise.resolve([]);
    });

    render(<MeetingHistory />);
    await flushSearchDebounce();
    expect(screen.getByRole("heading", { name: "Comité produit" })).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("Rechercher une réunion…"), {
      target: { value: "inexistant" },
    });
    await flushSearchDebounce();

    expect(screen.getByRole("heading", { name: "Aucun résultat" })).toBeInTheDocument();
    expect(screen.getByText(/Aucune réunion ne correspond à « inexistant »/)).toBeInTheDocument();
  });

  it("ouvre le détail au clic sur une réunion", async () => {
    invokeMock.mockImplementation((command: string) => {
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
            snippet: null,
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
          transcriptions: [
            { id: "t-1", meetingId: "m-1", createdAt: "", updatedAt: "" },
          ],
          summaries: [],
          actions: [],
        });
      }
      if (command === "get_transcription") {
        return Promise.resolve({
          id: "t-1",
          meetingId: "m-1",
          content: "Bonjour",
          createdAt: "",
          updatedAt: "",
        });
      }
      return Promise.resolve([]);
    });

    render(<MeetingHistory />);
    await flushSearchDebounce();

    const itemButton = screen.getByRole("button", { name: /Comité produit/ });
    fireEvent.click(itemButton);

    await waitFor(() => {
      expect(screen.getByText("← Retour à la liste")).toBeInTheDocument();
      expect(screen.getByText("Bonjour")).toBeInTheDocument();
    });
  });
});
