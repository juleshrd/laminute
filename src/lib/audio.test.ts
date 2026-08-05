import { describe, expect, it } from "vitest";
import { formatAudioError, formatDuration, isAudioError } from "./audio";

describe("audio helpers", () => {
  it("formats duration as mm:ss", () => {
    expect(formatDuration(0)).toBe("0:00");
    expect(formatDuration(65)).toBe("1:05");
    expect(formatDuration(null)).toBe("—");
  });

  it("detects structured audio errors", () => {
    expect(
      isAudioError({
        code: "permission_denied",
        message: "permission microphone refusée",
      }),
    ).toBe(true);
    expect(isAudioError("permission denied")).toBe(false);
  });

  it("formats structured audio errors as readable messages", () => {
    expect(
      formatAudioError({
        code: "unsupported_format",
        message: "format non supporté — seuls les fichiers MP3 sont acceptés pour l'import",
      }),
    ).toBe("format non supporté — seuls les fichiers MP3 sont acceptés pour l'import");
  });
});
