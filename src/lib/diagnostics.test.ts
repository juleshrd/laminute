import { describe, expect, it, vi } from "vitest";

import { captureRecoverableError, reportDiagnosticEvent } from "./diagnostics";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("diagnostics client", () => {
  it("reports recoverable errors without throwing", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await captureRecoverableError(new Error("boom sk-shouldStayInMessageButClipped"), "ui");
    expect(invokeMock).toHaveBeenCalledWith("report_diagnostic_event", {
      input: expect.objectContaining({
        code: "frontend_error",
        subsystem: "ui",
      }),
    });
  });

  it("forwards report_diagnostic_event payload", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await reportDiagnosticEvent({
      code: "db_error",
      message: "échec",
      subsystem: "db",
      correlationId: "c1",
    });
    expect(invokeMock).toHaveBeenCalledWith("report_diagnostic_event", {
      input: {
        code: "db_error",
        message: "échec",
        subsystem: "db",
        correlationId: "c1",
      },
    });
  });
});
