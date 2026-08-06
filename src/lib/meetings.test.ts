import { describe, expect, it } from "vitest";

import { formatDurationMs, meetingStatusLabel } from "./meetings";

describe("meetings", () => {
  it("affiche les libellés de statut en français", () => {
    expect(meetingStatusLabel("draft")).toBe("Brouillon");
    expect(meetingStatusLabel("completed")).toBe("Terminée");
  });

  it("formate une durée en minutes:secondes", () => {
    expect(formatDurationMs(125_000)).toBe("2:05");
    expect(formatDurationMs(null)).toBe("—");
  });
});
