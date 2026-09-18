import { describe, expect, it, vi } from "vitest";
import {
  CEF_CHECKING,
  CEF_UNAVAILABLE,
  formatCheckedAt,
  formatDenyEntry,
  formatPendingPromotion,
  sourceLabel,
} from "./cef-runtime";

describe("copy constants", () => {
  it("keeps the settings fallbacks", () => {
    expect(CEF_UNAVAILABLE).toBe("No disponible fuera de idioteque");
    expect(CEF_CHECKING).toBe("Buscando…");
  });
});

describe("sourceLabel", () => {
  it("labels the bundled slot as factory", () => {
    expect(sourceLabel("bundled")).toBe("de fábrica");
  });

  it("labels the installed slot as updated", () => {
    expect(sourceLabel("installed")).toBe("actualizado");
  });

  it("treats unknown, empty or differently-cased sources as factory", () => {
    expect(sourceLabel("downloaded")).toBe("de fábrica");
    expect(sourceLabel("")).toBe("de fábrica");
    expect(sourceLabel("Installed")).toBe("de fábrica");
    expect(sourceLabel("INSTALLED")).toBe("de fábrica");
  });
});

describe("formatCheckedAt", () => {
  it("says never when there is no timestamp", () => {
    expect(formatCheckedAt(null)).toBe("nunca");
  });

  it("says never when the timestamp is invalid", () => {
    expect(formatCheckedAt("no-es-una-fecha")).toBe("nunca");
  });

  it("says never for empty or whitespace timestamps", () => {
    expect(formatCheckedAt("")).toBe("nunca");
    expect(formatCheckedAt("   ")).toBe("nunca");
  });

  it("formats a timestamp as local dd/mm/aaaa hh:mm", () => {
    const iso = "2026-09-16T11:05:00.000Z";
    const date = new Date(iso);
    const expected = [
      String(date.getDate()).padStart(2, "0"),
      String(date.getMonth() + 1).padStart(2, "0"),
      String(date.getFullYear()),
    ].join("/") +
      " " +
      [
        String(date.getHours()).padStart(2, "0"),
        String(date.getMinutes()).padStart(2, "0"),
      ].join(":");
    expect(formatCheckedAt(iso)).toBe(expected);
  });

  it("uses local getters, not UTC", () => {
    vi.spyOn(Date.prototype, "getHours").mockReturnValue(1);
    vi.spyOn(Date.prototype, "getUTCHours").mockReturnValue(11);
    vi.spyOn(Date.prototype, "getMinutes").mockReturnValue(5);
    vi.spyOn(Date.prototype, "getUTCMinutes").mockReturnValue(5);
    try {
      expect(formatCheckedAt("2026-09-16T11:05:00.000Z").endsWith(" 01:05")).toBe(
        true,
      );
    } finally {
      vi.restoreAllMocks();
    }
  });
});

describe("formatDenyEntry", () => {
  it("joins chromium, reason and time", () => {
    const iso = "2026-10-01T10:00:00.000Z";
    expect(
      formatDenyEntry({
        cefVersion: "153.0.1+gabc",
        chromiumVersion: "153.0.8000.10",
        reason: "health-exit-10",
        at: iso,
      }),
    ).toBe(`153.0.8000.10 — health-exit-10 — ${formatCheckedAt(iso)}`);
  });

  it("still formats when the denylist timestamp is missing", () => {
    expect(
      formatDenyEntry({
        cefVersion: "153.0.1+gabc",
        chromiumVersion: "153.0.8000.10",
        reason: "health-timeout",
        at: "",
      }),
    ).toBe("153.0.8000.10 — health-timeout — nunca");
  });
});

describe("formatPendingPromotion", () => {
  it("names the Chromium waiting to be promoted", () => {
    expect(
      formatPendingPromotion({
        cefVersion: "153.0.1+gabc",
        chromiumVersion: "153.0.8000.10",
      }),
    ).toBe("Promoción pendiente: Chromium 153.0.8000.10");
  });
});
