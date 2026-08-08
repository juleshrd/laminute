import { describe, expect, it } from "vitest";

import { isOllamaLoopbackUrl } from "./ollamaUrl";

describe("isOllamaLoopbackUrl", () => {
  it("reconnaît le loopback", () => {
    expect(isOllamaLoopbackUrl("http://127.0.0.1:11434")).toBe(true);
    expect(isOllamaLoopbackUrl(" http://localhost:11434/ ")).toBe(true);
    expect(isOllamaLoopbackUrl("http://[::1]:11434")).toBe(true);
  });

  it("signale LAN / distant", () => {
    expect(isOllamaLoopbackUrl("http://192.168.1.10:11434")).toBe(false);
    expect(isOllamaLoopbackUrl("https://ollama.example.com")).toBe(false);
  });

  it("refuse les schémas non HTTP", () => {
    expect(isOllamaLoopbackUrl("file:///tmp")).toBe(false);
    expect(isOllamaLoopbackUrl("")).toBe(false);
  });
});
