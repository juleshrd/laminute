import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

import { MeetingHistory } from "./MeetingHistory";
import type { MeetingDetail, MeetingListItem, MeetingSearchPage } from "../lib/meetings";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  convertFileSrc: (path: string) => path,
}));

function meeting(overrides: Partial<MeetingListItem> = {}): MeetingListItem {
  return {
    id: "m-1",
    title: "Comité produit",
    status: "completed",
    startedAt: "2026-08-05T10:00:00Z",
    endedAt: null,
    createdAt: "2026-08-05T10:00:00Z",
    updatedAt: "2026-08-05T11:00:00Z",
    snippet: null,
    ...overrides,
  };
}

function detail(overrides: Partial<MeetingDetail> = {}): MeetingDetail {
  return {
    ...meeting(),
    description: null,
    audioFiles: [],
    transcriptions: [],
    summaries: [],
    actions: [],
    ...overrides,
  };
}

function searchPage(items: MeetingListItem[], nextCursor: string | null = null): MeetingSearchPage {
  return { items, nextCursor };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("MeetingHistory", () => {
  beforeEach(() => {
    cleanup();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "search_meetings") {
        return Promise.resolve(searchPage([meeting()]));
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
          return Promise.resolve(
            searchPage([
              meeting({
                id: "m-2",
                title: "Point commercial",
                snippet: "…Discussion avec le client Dufour sur…",
              }),
            ]),
          );
        }
        return Promise.resolve(searchPage([meeting()]));
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
          return Promise.resolve(searchPage([]));
        }
        return Promise.resolve(searchPage([meeting()]));
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
        return Promise.resolve(searchPage([meeting()]));
      }
      if (command === "get_meeting") {
        return Promise.resolve(
          detail({
            transcriptions: [
              { id: "t-1", meetingId: "m-1", content: "Bonjour", createdAt: "", updatedAt: "" },
            ],
          }),
        );
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

  it("charge la page suivante sans dupliquer les réunions déjà affichées", async () => {
    invokeMock.mockImplementation((command: string, args?: { filters?: { cursor?: string } }) => {
      if (command === "search_meetings") {
        if (args?.filters?.cursor === "cursor-2") {
          return Promise.resolve(
            searchPage([
              meeting({ id: "m-2", title: "Réunion 2" }),
              meeting({ id: "m-3", title: "Réunion 3" }),
            ]),
          );
        }
        return Promise.resolve(
          searchPage(
            [
              meeting({ id: "m-1", title: "Réunion 1" }),
              meeting({ id: "m-2", title: "Réunion 2" }),
            ],
            "cursor-2",
          ),
        );
      }
      return Promise.resolve([]);
    });

    render(<MeetingHistory />);

    expect(await screen.findByRole("heading", { name: "Réunion 1" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Charger plus de réunions" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("search_meetings", {
        filters: { cursor: "cursor-2" },
      });
    });
    expect(await screen.findByRole("heading", { name: "Réunion 3" })).toBeInTheDocument();
    expect(screen.getAllByRole("heading", { level: 3 })).toHaveLength(3);
  });

  it("ignore une ancienne recherche qui se résout après la dernière intention", async () => {
    const oldSearch = deferred<ReturnType<typeof searchPage>>();
    const newSearch = deferred<ReturnType<typeof searchPage>>();

    invokeMock.mockImplementation((command: string, args?: { filters?: { query?: string } }) => {
      if (command === "search_meetings") {
        if (args?.filters?.query === "ancien") {
          return oldSearch.promise;
        }
        if (args?.filters?.query === "nouveau") {
          return newSearch.promise;
        }
        return Promise.resolve(searchPage([]));
      }
      return Promise.resolve([]);
    });

    render(<MeetingHistory />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("search_meetings", { filters: {} }),
    );

    fireEvent.change(screen.getByPlaceholderText("Rechercher une réunion…"), {
      target: { value: "ancien" },
    });
    await flushSearchDebounce();
    fireEvent.change(screen.getByPlaceholderText("Rechercher une réunion…"), {
      target: { value: "nouveau" },
    });
    await flushSearchDebounce();

    await act(async () => {
      newSearch.resolve(searchPage([meeting({ id: "new", title: "Résultat nouveau" })]));
      await Promise.resolve();
    });

    expect(await screen.findByRole("heading", { name: "Résultat nouveau" })).toBeInTheDocument();

    await act(async () => {
      oldSearch.resolve(searchPage([meeting({ id: "old", title: "Résultat ancien" })]));
      await Promise.resolve();
    });

    expect(screen.getByRole("heading", { name: "Résultat nouveau" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Résultat ancien" })).not.toBeInTheDocument();
  });

  it("ignore le détail d'une réunion précédemment sélectionnée", async () => {
    const meetingA = deferred<ReturnType<typeof detail>>();
    const meetingB = deferred<ReturnType<typeof detail>>();

    invokeMock.mockImplementation((command: string, args?: { id?: string }) => {
      if (command === "search_meetings") {
        return Promise.resolve(
          searchPage([
            meeting({ id: "a", title: "Réunion A" }),
            meeting({ id: "b", title: "Réunion B" }),
          ]),
        );
      }
      if (command === "get_meeting") {
        return args?.id === "a" ? meetingA.promise : meetingB.promise;
      }
      return Promise.resolve([]);
    });

    render(<MeetingHistory />);
    expect(await screen.findByRole("heading", { name: "Réunion A" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Réunion A/ }));
    fireEvent.click(screen.getByRole("button", { name: /Réunion B/ }));

    await act(async () => {
      meetingA.resolve(
        detail({
          id: "a",
          title: "Réunion A",
          transcriptions: [
            { id: "ta", meetingId: "a", content: "Contenu A", createdAt: "", updatedAt: "" },
          ],
        }),
      );
      await Promise.resolve();
    });

    expect(screen.queryByText("Contenu A")).not.toBeInTheDocument();
    expect(screen.getByText("Chargement du détail…")).toBeInTheDocument();

    await act(async () => {
      meetingB.resolve(
        detail({
          id: "b",
          title: "Réunion B",
          transcriptions: [
            { id: "tb", meetingId: "b", content: "Contenu B", createdAt: "", updatedAt: "" },
          ],
        }),
      );
      await Promise.resolve();
    });

    expect(await screen.findByText("Contenu B")).toBeInTheDocument();
    expect(screen.queryByText("Contenu A")).not.toBeInTheDocument();
  });
});
