import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  applyReduceMotionToDocument,
  getReduceMotionPreference,
  setReduceMotionPreference,
} from "./preferences";

describe("reduce motion preference", () => {
  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.reduceMotion;
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    localStorage.clear();
    delete document.documentElement.dataset.reduceMotion;
  });

  it("suit la préférence OS quand rien n'est stocké", () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn().mockImplementation((query: string) => ({
        matches: query === "(prefers-reduced-motion: reduce)",
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );

    expect(getReduceMotionPreference()).toBe(true);
    applyReduceMotionToDocument();
    expect(document.documentElement.dataset.reduceMotion).toBe("true");
  });

  it("respecte une préférence applicative explicite (activée)", () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );

    setReduceMotionPreference(true);
    expect(localStorage.getItem("laminute.reduceMotion")).toBe("1");
    expect(getReduceMotionPreference()).toBe(true);
    expect(document.documentElement.dataset.reduceMotion).toBe("true");
  });

  it("respecte une préférence applicative explicite (désactivée) malgré l'OS", () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn().mockImplementation((query: string) => ({
        matches: query === "(prefers-reduced-motion: reduce)",
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );

    setReduceMotionPreference(false);
    expect(localStorage.getItem("laminute.reduceMotion")).toBe("0");
    expect(getReduceMotionPreference()).toBe(false);
    expect(document.documentElement.dataset.reduceMotion).toBe("false");
  });

  it("persiste via applyReduceMotionToDocument", () => {
    localStorage.setItem("laminute.reduceMotion", "1");
    applyReduceMotionToDocument();
    expect(document.documentElement.dataset.reduceMotion).toBe("true");

    localStorage.setItem("laminute.reduceMotion", "0");
    applyReduceMotionToDocument();
    expect(document.documentElement.dataset.reduceMotion).toBe("false");
  });
});
