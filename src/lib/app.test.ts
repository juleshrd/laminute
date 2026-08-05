import { describe, expect, it } from "vitest";
import { APP_IDENTIFIER, APP_NAME } from "./app";

describe("app metadata", () => {
  it("exposes the product name", () => {
    expect(APP_NAME).toBe("La Minute");
  });

  it("exposes the desktop identifier", () => {
    expect(APP_IDENTIFIER).toBe("app.laminute.desktop");
  });
});
